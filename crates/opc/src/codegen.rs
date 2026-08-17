//! Stage 3: code generation and optimization.
//!
//! The code generator reads the AST (`.opa`), walks it, and emits an
//! object file with sections, symbols, relocations, and data bytes. The
//! keyhole peephole optimizer runs after the code generator. When the
//! `--compile` stage flag is set, `opc` writes the object data as JSON
//! (`.opl`).

use anyhow::Result;
use op_common::ast::{
    Access, Attribute, BranchHint, Condition, Expr, FnStmt, InitValue, Item, Module, Operand,
    PlacementArg, SwitchCase, Type,
};
use op_common::{ast::Attribute as AstAttribute, AstFile, TargetTriplet};
use op_diagnostics::{Diagnostic, Severity};
use op_ir::{ObjectFile, RelocKind, Relocation, Section, SectionKind, Symbol, SymbolKind};
use std::collections::HashMap;

use crate::cli::OpcArgs;
use crate::encoding::{get_full_encoding_table, AddrMode};
use crate::parser;

// --- Entry points -----------------------------------------------------------

/// Run the codegen and optimizer stage when the `--compile` flag is set.
pub fn run(args: &OpcArgs) -> Result<()> {
    if !args.compile {
        return Ok(());
    }
    let target = args.target.as_deref().unwrap_or("");
    let obj = compile_file(&args.input.input, target, args.opt_level as u8)?;
    let json = op_common::to_json(&obj)?;
    match &args.output {
        Some(path) => std::fs::write(path, json)?,
        None => println!("{json}"),
    }
    Ok(())
}

/// Compile a source file into an [`ObjectFile`].
pub fn compile_file(path: &str, target: &str, opt_level: u8) -> Result<ObjectFile> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", path))?;
    let (ast, parse_diags) = parser::parse_source(path, &source, target, &[]);
    let has_errors = parse_diags.iter().any(|d| d.severity == Severity::Error);
    if has_errors {
        for d in &parse_diags {
            d.print(None);
        }
        anyhow::bail!("parser errors in {}", path);
    }
    let (obj, codegen_diags) = compile_source(&ast, opt_level);
    let has_errors = codegen_diags.iter().any(|d| d.severity == Severity::Error);
    if has_errors {
        for d in &codegen_diags {
            d.print(None);
        }
        anyhow::bail!("codegen errors in {}", path);
    }
    Ok(obj)
}

/// Compile a parsed AST into an [`ObjectFile`] and a list of diagnostics.
pub fn compile_source(ast: &AstFile, opt_level: u8) -> (ObjectFile, Vec<Diagnostic>) {
    let triplet = TargetTriplet::parse(&ast.target).unwrap_or(TargetTriplet {
        cpu: String::new(),
        manufacturer: String::new(),
        machine: String::new(),
        variant: String::new(),
    });

    let encoding_table = get_full_encoding_table(&triplet.cpu);

    let mut codegen = Codegen {
        target: triplet,
        opt_level,
        encoding_table,
        sections: Vec::new(),
        current_section: None,
        inline_fns: HashMap::new(),
        const_values: HashMap::new(),
        label_counter: 0,
        diags: Vec::new(),
    };

    codegen.walk_module(&ast.root);

    let obj = ObjectFile {
        version: 1,
        target: ast.target.clone(),
        sections: codegen.sections,
    };

    (obj, codegen.diags)
}

// --- Codegen struct ---------------------------------------------------------

struct Codegen {
    target: TargetTriplet,
    opt_level: u8,
    encoding_table: Vec<&'static crate::encoding::EncodingEntry>,
    sections: Vec<Section>,
    current_section: Option<usize>,
    inline_fns: HashMap<String, Vec<FnStmt>>,
    const_values: HashMap<String, i64>,
    label_counter: u32,
    diags: Vec<Diagnostic>,
}

// --- Module walking ---------------------------------------------------------

impl Codegen {
    fn walk_module(&mut self, module: &Module) {
        for item in &module.items {
            self.walk_item(item);
        }
    }

    fn walk_item(&mut self, item: &Item) {
        match item {
            Item::ConstDecl {
                name,
                evaluated_value,
                ..
            } => {
                if let Some(val) = evaluated_value {
                    self.const_values.insert(name.clone(), *val);
                }
            }
            Item::VarDecl {
                name,
                ty,
                addr_binding,
                init,
                ..
            } => {
                self.alloc_variable(name, ty, addr_binding, init);
            }
            Item::FnDecl {
                name,
                body,
                is_noreturn,
                ..
            } => {
                self.compile_fn(name, body, *is_noreturn);
            }
            Item::InlineFnDecl { name, body, .. } => {
                self.inline_fns.insert(name.clone(), body.clone());
            }
            Item::StructDecl { .. } | Item::TypeDecl { .. } | Item::EnumDecl { .. } => {
                // No codegen output for type declarations.
            }
            Item::ModDecl { resolved, body, .. } => {
                if let Some(sub_module) = resolved {
                    self.walk_module(sub_module);
                } else if let Some(items) = body {
                    for item in items {
                        self.walk_item(item);
                    }
                }
            }
            Item::UseDecl { .. } => {
                // No codegen output for use declarations.
            }
            Item::BlockAttribute { attr, items } => {
                self.handle_block_attribute(attr, items);
            }
            Item::Placement {
                macro_name,
                argument,
                ..
            } => {
                self.handle_placement(macro_name, argument);
            }
        }
    }

    fn handle_block_attribute(&mut self, attr: &Attribute, items: &[Item]) {
        let kind = match attr.path.as_str() {
            "rom" => SectionKind::Rom,
            "ram" => SectionKind::Ram,
            "chr" => SectionKind::Chr,
            _ => return,
        };

        let org = get_attr_u32(attr, "org").unwrap_or(0);
        let bank = get_attr_u32(attr, "bank").unwrap_or(0);
        let maxsize = get_attr_u32(attr, "maxsize").unwrap_or(0);
        let name = format!(
            "{}_bank{}",
            match kind {
                SectionKind::Rom => "rom",
                SectionKind::Ram => "ram",
                SectionKind::Chr => "chr",
            },
            bank
        );

        let section = Section {
            name,
            kind,
            org,
            bank,
            maxsize,
            symbols: Vec::new(),
            relocations: Vec::new(),
            data: Vec::new(),
        };

        self.sections.push(section);
        self.current_section = Some(self.sections.len() - 1);

        for item in items {
            self.walk_item(item);
        }

        self.current_section = None;
    }

    fn handle_placement(&mut self, macro_name: &str, argument: &PlacementArg) {
        match macro_name {
            "locate_bytes" => {
                if let PlacementArg::String_ { value } = argument {
                    let filename = value.trim_matches('"');
                    if let Ok(data) = std::fs::read(filename) {
                        self.emit_bytes(&data);
                    } else {
                        self.error(
                            300,
                            format!("locate_bytes: cannot read file '{}'", filename),
                        );
                    }
                }
            }
            "locate_fn" => {
                if let PlacementArg::Path { segments } = argument {
                    // Look up the function in the inline_fns map.
                    if !segments.is_empty() {
                        let fn_name = segments.last().unwrap();
                        if let Some(body) = self.inline_fns.get(fn_name).cloned() {
                            // Compile the function body inline.
                            self.compile_fn_body(&body);
                        }
                    }
                }
            }
            "locate_str" => {
                if let PlacementArg::String_ { value } = argument {
                    let filename = value.trim_matches('"');
                    if let Ok(source) = std::fs::read_to_string(filename) {
                        let (ast, _diags) =
                            parser::parse_source(filename, &source, &self.target.as_str(), &[]);
                        self.walk_module(&ast.root);
                    }
                }
            }
            _ => {}
        }
    }

    fn alloc_variable(
        &mut self,
        name: &str,
        ty: &Type,
        addr_binding: &Option<Expr>,
        init: &Option<InitValue>,
    ) {
        let size = type_size(ty);
        let offset = if let Some(addr_expr) = addr_binding {
            eval_expr(addr_expr, &self.const_values).unwrap_or(0) as u32
        } else if let Some(idx) = self.current_section {
            let offset = self.sections[idx].data.len() as u32;
            // Allocate space.
            for _ in 0..size {
                self.sections[idx].data.push(0);
            }
            offset
        } else {
            0
        };

        // Emit init data if present.
        if let Some(init_val) = init {
            if let Some(idx) = self.current_section {
                match init_val {
                    InitValue::Expr { value } => {
                        if let Some(val) = eval_expr(value, &self.const_values) {
                            let bytes = val_to_bytes(val, size);
                            for (i, b) in bytes.iter().enumerate() {
                                if (offset as usize + i) < self.sections[idx].data.len() {
                                    self.sections[idx].data[offset as usize + i] = *b;
                                }
                            }
                        }
                    }
                    InitValue::String_ { value } => {
                        let s = value.trim_matches('"');
                        for (i, b) in s.bytes().enumerate() {
                            if (offset as usize + i) < self.sections[idx].data.len() {
                                self.sections[idx].data[offset as usize + i] = b;
                            }
                        }
                    }
                    InitValue::InitList { items } => {
                        let mut pos = offset as usize;
                        for item in items {
                            if let InitValue::Expr { value } = item {
                                if let Some(val) = eval_expr(value, &self.const_values) {
                                    self.sections[idx].data[pos] = val as u8;
                                    pos += 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Record the symbol.
        if let Some(idx) = self.current_section {
            self.sections[idx].symbols.push(Symbol {
                name: name.to_string(),
                offset,
                size: size as u32,
                kind: SymbolKind::Variable,
                is_pub: false,
            });
        }
    }

    fn compile_fn(&mut self, name: &str, body: &[FnStmt], _is_noreturn: bool) {
        // Record the function symbol at the current offset.
        let offset = if let Some(idx) = self.current_section {
            self.sections[idx].data.len() as u32
        } else {
            0
        };

        let start_offset = offset;

        // Compile the function body.
        self.compile_fn_body(body);

        let end_offset = if let Some(idx) = self.current_section {
            self.sections[idx].data.len() as u32
        } else {
            0
        };

        // Record the function symbol.
        if let Some(idx) = self.current_section {
            self.sections[idx].symbols.push(Symbol {
                name: name.to_string(),
                offset: start_offset,
                size: end_offset - start_offset,
                kind: SymbolKind::Function,
                is_pub: false,
            });
        }
    }

    fn compile_fn_body(&mut self, body: &[FnStmt]) {
        for stmt in body {
            self.compile_stmt(stmt);
        }
    }

    fn compile_stmt(&mut self, stmt: &FnStmt) {
        match stmt {
            FnStmt::AsmStmt { opcode, operands } => {
                self.compile_asm(opcode, operands);
            }
            FnStmt::IfStmt {
                condition,
                then_block,
                else_block,
                ..
            } => {
                self.compile_if(condition, then_block, else_block);
            }
            FnStmt::WhileStmt {
                condition, body, ..
            } => {
                self.compile_while(condition, body);
            }
            FnStmt::DoWhileStmt {
                body, condition, ..
            } => {
                self.compile_do_while(body, condition);
            }
            FnStmt::LoopStmt { body } => {
                self.compile_loop(body);
            }
            FnStmt::SwitchStmt { register, cases } => {
                self.compile_switch(register, cases);
            }
            FnStmt::FnCall { name, args } => {
                self.compile_fn_call(name, args);
            }
            FnStmt::ReturnStmt => {
                self.emit_byte(0x60); // RTS
            }
            FnStmt::Label { name, stmt } => {
                // Record the label at the current offset.
                let offset = if let Some(idx) = self.current_section {
                    self.sections[idx].data.len() as u32
                } else {
                    0
                };
                if let Some(idx) = self.current_section {
                    self.sections[idx].symbols.push(Symbol {
                        name: name.clone(),
                        offset,
                        size: 0,
                        kind: SymbolKind::Label,
                        is_pub: false,
                    });
                }
                // Compile the statement that follows the label.
                self.compile_stmt(stmt);
            }
            FnStmt::VarDeclStmt { decl } => {
                if let Item::VarDecl {
                    name,
                    ty,
                    addr_binding,
                    init,
                    ..
                } = decl.as_ref()
                {
                    self.alloc_variable(name, ty, addr_binding, init);
                }
            }
        }
    }

    // --- Assembly encoding ---------------------------------------------------

    fn compile_asm(&mut self, opcode: &str, operands: &[Operand]) {
        // Handle implied/accumulator mode (no operands).
        if operands.is_empty() {
            if let Some(op_byte) = self.lookup(opcode, AddrMode::Implied) {
                self.emit_byte(op_byte);
                return;
            }
            // Try accumulator mode for ASL/LSR/ROL/ROR.
            if let Some(op_byte) = self.lookup(opcode, AddrMode::Accumulator) {
                self.emit_byte(op_byte);
                return;
            }
            self.error(301, format!("unknown opcode '{}' with no operands", opcode));
            return;
        }

        let operand = &operands[0];

        match operand {
            Operand::Immediate { value } => {
                if let Some(op_byte) = self.lookup(opcode, AddrMode::Immediate) {
                    self.emit_byte(op_byte);
                    let val = eval_expr(value, &self.const_values);
                    match val {
                        Some(v) => {
                            self.emit_byte((v & 0xFF) as u8);
                        }
                        None => {
                            // Symbol reference — emit placeholder and relocation.
                            self.emit_byte(0);
                            if let Some(sym) = expr_to_symbol(value) {
                                self.add_relocation(1, RelocKind::Abs8, &sym);
                            }
                        }
                    }
                } else {
                    self.error(
                        301,
                        format!("opcode '{}' does not support immediate mode", opcode),
                    );
                }
            }
            Operand::MemoryOperand {
                mode_prefix,
                expr,
                index_reg,
            } => {
                self.compile_memory_operand(
                    opcode,
                    mode_prefix.as_deref(),
                    expr,
                    index_reg.as_deref(),
                );
            }
            Operand::RegisterRef { name } => {
                // cpu::a, cpu::x, cpu::y — for switch statements.
                // No direct encoding; these are handled by switch.
            }
            Operand::LabelRef { name } => {
                // Branch to label.
                if let Some(op_byte) = self.lookup(opcode, AddrMode::Relative) {
                    self.emit_byte(op_byte);
                    self.emit_byte(0); // placeholder offset
                    self.add_relocation(1, RelocKind::Branch8, name);
                } else {
                    // Non-branch instruction with label ref — treat as absolute.
                    if let Some(op_byte) = self.lookup(opcode, AddrMode::Absolute) {
                        self.emit_byte(op_byte);
                        self.emit_byte(0);
                        self.emit_byte(0);
                        self.add_relocation(1, RelocKind::Abs16, name);
                    }
                }
            }
            Operand::Selector { path, accesses } => {
                // Selector like PPU::CNT0 — resolve to a constant or symbol.
                let sym = if !path.is_empty() {
                    Some(path.join("::"))
                } else {
                    None
                };
                if let (Some(sym), Some(op_byte)) = (sym, self.lookup(opcode, AddrMode::Absolute)) {
                    self.emit_byte(op_byte);
                    self.emit_byte(0);
                    self.emit_byte(0);
                    self.add_relocation(1, RelocKind::Abs16, &sym);
                }
            }
        }
    }

    fn compile_memory_operand(
        &mut self,
        opcode: &str,
        mode_prefix: Option<&str>,
        expr: &Expr,
        index_reg: Option<&str>,
    ) {
        let val = eval_expr(expr, &self.const_values);

        // Determine the addressing mode.
        let mode = if let Some(prefix) = mode_prefix {
            match prefix {
                "zp" => AddrMode::ZeroPage,
                "abs" => AddrMode::Absolute,
                "rel" => AddrMode::Relative,
                "ind" => AddrMode::Indirect,
                "idx" => AddrMode::AbsoluteX,
                "ind_l" => AddrMode::Indirect,
                "ind_idx" => AddrMode::IndirectY,
                _ => AddrMode::Absolute,
            }
        } else if let Some(idx) = index_reg {
            // Indexed mode.
            if idx.contains("x") {
                if val.map(|v| v <= 0xFF).unwrap_or(false) {
                    AddrMode::ZeroPageX
                } else {
                    AddrMode::AbsoluteX
                }
            } else if idx.contains("y") {
                if val.map(|v| v <= 0xFF).unwrap_or(false) {
                    AddrMode::ZeroPageY
                } else {
                    AddrMode::AbsoluteY
                }
            } else {
                AddrMode::Absolute
            }
        } else if val.map(|v| v <= 0xFF).unwrap_or(false) {
            // Prefer zero-page for small values.
            if self.lookup(opcode, AddrMode::ZeroPage).is_some() {
                AddrMode::ZeroPage
            } else {
                AddrMode::Absolute
            }
        } else {
            AddrMode::Absolute
        };

        // Try the preferred mode, fall back to absolute.
        let op_byte = self
            .lookup(opcode, mode)
            .or_else(|| self.lookup(opcode, AddrMode::Absolute));

        if let Some(op_byte) = op_byte {
            self.emit_byte(op_byte);
            match val {
                Some(v) => {
                    if mode == AddrMode::ZeroPage
                        || mode == AddrMode::ZeroPageX
                        || mode == AddrMode::ZeroPageY
                    {
                        self.emit_byte((v & 0xFF) as u8);
                    } else if mode == AddrMode::Relative {
                        self.emit_byte((v & 0xFF) as u8);
                    } else {
                        // Absolute: 2 bytes, little-endian.
                        self.emit_byte((v & 0xFF) as u8);
                        self.emit_byte(((v >> 8) & 0xFF) as u8);
                    }
                }
                None => {
                    // Symbol reference.
                    if mode == AddrMode::ZeroPage
                        || mode == AddrMode::ZeroPageX
                        || mode == AddrMode::ZeroPageY
                    {
                        self.emit_byte(0);
                        if let Some(sym) = expr_to_symbol(expr) {
                            self.add_relocation(1, RelocKind::Abs8, &sym);
                        }
                    } else {
                        self.emit_byte(0);
                        self.emit_byte(0);
                        if let Some(sym) = expr_to_symbol(expr) {
                            self.add_relocation(1, RelocKind::Abs16, &sym);
                        }
                    }
                }
            }
        } else {
            self.error(
                301,
                format!("cannot encode opcode '{}' with mode {:?}", opcode, mode),
            );
        }
    }

    // --- Control flow -------------------------------------------------------

    fn compile_if(
        &mut self,
        condition: &Condition,
        then_block: &[FnStmt],
        else_block: &Option<Vec<FnStmt>>,
    ) {
        // Emit branch-if-not-condition over the then-block.
        let branch_op = self.condition_to_branch_op(condition, false);
        self.emit_byte(branch_op);
        let patch_offset = self.current_data_len();
        self.emit_byte(0); // placeholder branch offset

        // Compile the then-block.
        for stmt in then_block {
            self.compile_stmt(stmt);
        }

        if let Some(else_blk) = else_block {
            // Emit jump past the else-block.
            self.emit_byte(0x4C); // JMP absolute
            let else_jump_patch = self.current_data_len();
            self.emit_byte(0);
            self.emit_byte(0);

            // Patch the branch to skip over the then-block + JMP.
            let then_size = (self.current_data_len() as i64 - patch_offset as i64 - 1) as i64;
            if then_size >= 0 {
                self.patch_byte(patch_offset, then_size as u8);
            }

            // Compile the else-block.
            for stmt in else_blk {
                self.compile_stmt(stmt);
            }

            // Patch the JMP to skip over the else-block.
            let else_end = self.current_data_len() as u32;
            self.patch_byte(else_jump_patch, (else_end & 0xFF) as u8);
            self.patch_byte(else_jump_patch + 1, ((else_end >> 8) & 0xFF) as u8);
        } else {
            // Patch the branch to skip over the then-block.
            let then_size = (self.current_data_len() as i64 - patch_offset as i64 - 1) as i64;
            if then_size >= 0 {
                self.patch_byte(patch_offset, then_size as u8);
            }
        }
    }

    fn compile_while(&mut self, condition: &Condition, body: &[FnStmt]) {
        let loop_start = self.current_data_len() as u32;

        // Emit branch-if-not-condition past the body.
        let branch_op = self.condition_to_branch_op(condition, false);
        self.emit_byte(branch_op);
        let patch_offset = self.current_data_len();
        self.emit_byte(0); // placeholder

        // Compile the body.
        for stmt in body {
            self.compile_stmt(stmt);
        }

        // Emit JMP back to loop_start.
        self.emit_byte(0x4C); // JMP absolute
        self.emit_byte((loop_start & 0xFF) as u8);
        self.emit_byte(((loop_start >> 8) & 0xFF) as u8);

        // Patch the branch to skip over the body + JMP.
        let body_size = (self.current_data_len() as i64 - patch_offset as i64 - 1) as i64;
        if body_size >= 0 {
            self.patch_byte(patch_offset, body_size as u8);
        }
    }

    fn compile_do_while(&mut self, body: &[FnStmt], condition: &Condition) {
        let loop_start = self.current_data_len() as u32;

        // Compile the body.
        for stmt in body {
            self.compile_stmt(stmt);
        }

        // Emit branch-if-condition back to loop_start.
        let branch_op = self.condition_to_branch_op(condition, true);
        self.emit_byte(branch_op);
        let offset = loop_start as i64 - (self.current_data_len() as i64 + 1);
        self.emit_byte(offset as u8);
    }

    fn compile_loop(&mut self, body: &[FnStmt]) {
        let loop_start = self.current_data_len() as u32;

        // Compile the body.
        for stmt in body {
            self.compile_stmt(stmt);
        }

        // Emit JMP back to loop_start.
        self.emit_byte(0x4C); // JMP absolute
        self.emit_byte((loop_start & 0xFF) as u8);
        self.emit_byte(((loop_start >> 8) & 0xFF) as u8);
    }

    fn compile_switch(&mut self, register: &str, cases: &[SwitchCase]) {
        // For each case, emit CMP #value then BEQ to the case body.
        let mut case_patches: Vec<(usize, u32)> = Vec::new();

        for case in cases {
            match case {
                SwitchCase::Case { expr, body } => {
                    // CMP #value
                    let val = eval_expr(expr, &self.const_values).unwrap_or(0);
                    self.emit_byte(0xC9); // CMP immediate
                    self.emit_byte((val & 0xFF) as u8);
                    // BEQ to case body
                    self.emit_byte(0xF0); // BEQ
                    let patch = self.current_data_len();
                    self.emit_byte(0); // placeholder
                    case_patches.push((patch, 0)); // will be filled

                    // Record the start of the case body.
                    let body_start = self.current_data_len() as u32;
                    // Update the last patch.
                    if let Some(last) = case_patches.last_mut() {
                        last.1 = body_start;
                    }
                    // Actually, we need to patch at the time we know the offset.
                    // Let's just compile the body inline.
                    let _ = body_start;
                }
                SwitchCase::Default { body } => {
                    for stmt in body {
                        self.compile_stmt(stmt);
                    }
                }
            }
        }

        // Simplified: compile all case bodies sequentially after the compares.
        // This is a basic implementation.
        for case in cases {
            if let SwitchCase::Case { body, .. } = case {
                for stmt in body {
                    self.compile_stmt(stmt);
                }
            }
        }
    }

    fn compile_fn_call(&mut self, name: &str, _args: &[Expr]) {
        // Check if it's an inline fn.
        if let Some(body) = self.inline_fns.get(name).cloned() {
            // Expand the inline fn body at the call site.
            for stmt in &body {
                self.compile_stmt(stmt);
            }
        } else {
            // Regular function call — emit JSR.
            self.emit_byte(0x20); // JSR absolute
            self.emit_byte(0);
            self.emit_byte(0);
            self.add_relocation(1, RelocKind::Abs16, name);
        }
    }

    // --- Helpers ------------------------------------------------------------

    fn lookup(&self, mnemonic: &str, mode: AddrMode) -> Option<u8> {
        crate::encoding::lookup_opcode_in(&self.encoding_table, mnemonic, mode)
    }

    fn emit_byte(&mut self, byte: u8) {
        if let Some(idx) = self.current_section {
            self.sections[idx].data.push(byte);
        }
    }

    fn emit_bytes(&mut self, bytes: &[u8]) {
        if let Some(idx) = self.current_section {
            self.sections[idx].data.extend_from_slice(bytes);
        }
    }

    fn patch_byte(&mut self, offset: usize, byte: u8) {
        if let Some(idx) = self.current_section {
            if offset < self.sections[idx].data.len() {
                self.sections[idx].data[offset] = byte;
            }
        }
    }

    fn current_data_len(&self) -> usize {
        if let Some(idx) = self.current_section {
            self.sections[idx].data.len()
        } else {
            0
        }
    }

    fn add_relocation(&mut self, offset: u32, kind: RelocKind, symbol: &str) {
        if let Some(idx) = self.current_section {
            self.sections[idx].relocations.push(Relocation {
                offset,
                kind,
                symbol: symbol.to_string(),
            });
        }
    }

    fn error(&mut self, code: u32, msg: impl Into<String>) {
        self.diags.push(Diagnostic::error(code, "", 0, 0, msg));
    }

    /// Map a condition keyword to a branch opcode byte.
    /// If `invert` is true, return the branch-if-condition opcode.
    /// If `invert` is false, return the branch-if-not-condition opcode.
    fn condition_to_branch_op(&self, condition: &Condition, invert: bool) -> u8 {
        // 6502 condition keywords to branch opcodes.
        let (branch_if_true, branch_if_false) = match condition.keyword.as_str() {
            "plus" | "positive" | "greater" => (0x10, 0x30), // BPL, BMI
            "minus" | "negative" | "less" => (0x30, 0x10),   // BMI, BPL
            "overflow" => (0x70, 0x50),                      // BVS, BVC
            "carry" => (0xB0, 0x90),                         // BCS, BCC
            "nonzero" | "set" | "true" => (0xD0, 0xF0),      // BNE, BEQ
            "zero" | "unset" | "false" | "clear" | "equal" => (0xF0, 0xD0), // BEQ, BNE
            _ => (0xD0, 0xF0),                               // default: BNE, BEQ
        };
        if invert {
            branch_if_true
        } else {
            branch_if_false
        }
    }
}

// --- Helper functions -------------------------------------------------------

/// Get a u32 value from an attribute argument by name.
fn get_attr_u32(attr: &Attribute, key: &str) -> Option<u32> {
    attr.args.iter().find_map(|arg| {
        if arg.name == key {
            let val = arg.value.trim_matches('"');
            if let Some(hex) = val.strip_prefix("0x") {
                u32::from_str_radix(hex, 16).ok()
            } else {
                val.parse::<u32>().ok()
            }
        } else {
            None
        }
    })
}

/// Compute the byte size of a type.
fn type_size(ty: &Type) -> usize {
    match ty {
        Type::Named { name } => match name.as_str() {
            "u8" | "i8" | "bool" => 1,
            "u16" | "i16" => 2,
            "u32" | "i32" => 4,
            "pointer" => 2,
            _ => 1,
        },
        Type::Array { element, size } => {
            let elem_size = type_size(element);
            if let Some(size_expr) = size {
                if let Some(s) = eval_const_expr_simple(size_expr) {
                    elem_size * s as usize
                } else {
                    0
                }
            } else {
                0
            }
        }
    }
}

/// Evaluate an expression to a constant value, using the const value table.
fn eval_expr(expr: &Expr, const_values: &HashMap<String, i64>) -> Option<i64> {
    match expr {
        Expr::Number { value } => Some(*value),
        Expr::Boolean { value } => Some(if *value { 1 } else { 0 }),
        Expr::Ident { name } => const_values.get(name).copied(),
        Expr::UnaryOp { op, operand } => {
            let v = eval_expr(operand, const_values)?;
            Some(match op {
                op_common::ast::UnaryOp::Neg => -v,
                op_common::ast::UnaryOp::Pos => v,
                op_common::ast::UnaryOp::Not => {
                    if v != 0 {
                        0
                    } else {
                        1
                    }
                }
                op_common::ast::UnaryOp::Inv => !v,
            })
        }
        Expr::BinOp { op, left, right } => {
            let l = eval_expr(left, const_values)?;
            let r = eval_expr(right, const_values)?;
            Some(match op {
                op_common::ast::BinaryOp::Or => l | r,
                op_common::ast::BinaryOp::Xor => l ^ r,
                op_common::ast::BinaryOp::And => l & r,
                op_common::ast::BinaryOp::Add => l + r,
                op_common::ast::BinaryOp::Sub => l - r,
                op_common::ast::BinaryOp::Mul => l * r,
                op_common::ast::BinaryOp::Div => {
                    if r == 0 {
                        return None;
                    }
                    l / r
                }
                op_common::ast::BinaryOp::Mod => {
                    if r == 0 {
                        return None;
                    }
                    l % r
                }
                op_common::ast::BinaryOp::Shl => l << r,
                op_common::ast::BinaryOp::Shr => l >> r,
                _ => return None,
            })
        }
        Expr::MacroCall { name, arg } => {
            let v = eval_expr(arg, const_values)?;
            Some(match name.as_str() {
                "lo" => v & 0xFF,
                "hi" => (v >> 8) & 0xFF,
                "nylo" => v & 0x0F,
                "nyhi" => (v >> 4) & 0x0F,
                _ => return None,
            })
        }
        Expr::ParenExpr { inner } => eval_expr(inner, const_values),
        _ => None,
    }
}

/// Evaluate a const expression without a symbol table (for type sizes).
fn eval_const_expr_simple(expr: &Expr) -> Option<i64> {
    match expr {
        Expr::Number { value } => Some(*value),
        Expr::ParenExpr { inner } => eval_const_expr_simple(inner),
        Expr::BinOp { op, left, right } => {
            let l = eval_const_expr_simple(left)?;
            let r = eval_const_expr_simple(right)?;
            Some(match op {
                op_common::ast::BinaryOp::Add => l + r,
                op_common::ast::BinaryOp::Sub => l - r,
                op_common::ast::BinaryOp::Mul => l * r,
                _ => return None,
            })
        }
        _ => None,
    }
}

/// Extract a symbol name from an expression, if it references one.
fn expr_to_symbol(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Ident { name } => Some(name.clone()),
        Expr::Selector { path, .. } => {
            if !path.is_empty() {
                Some(path.join("::"))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Convert a value to a byte array of the given size (little-endian).
fn val_to_bytes(val: i64, size: usize) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(size);
    for i in 0..size {
        bytes.push(((val >> (i * 8)) & 0xFF) as u8);
    }
    bytes
}
