//! Stage 3: code generation and optimization.
//!
//! The code generator reads the AST (`.opa`), walks it, and emits an
//! object file with sections, symbols, relocations, and data bytes. The
//! keyhole peephole optimizer runs after the code generator. When the
//! `--compile` stage flag is set, `opc` writes the object data as JSON
//! (`.opl`).

use anyhow::Result;
use op_common::ast::{
    Access, Attribute, Condition, Expr, FnStmt, InitValue, Item, Module, OffsetOp, Operand,
    PlacementArg, SwitchCase, Type, UseRoot, UseTail, UseTree,
};
use op_common::{AstFile, TargetTriplet};
use op_diagnostics::{Diagnostic, Severity};
use op_ir::{ObjectFile, RelocKind, Relocation, Section, SectionKind, Symbol, SymbolKind};
use std::collections::{HashMap, HashSet};

use crate::cli::OpcArgs;
use crate::encoding::{get_full_encoding_table, AddrMode};
use crate::parser;

// --- Entry points -----------------------------------------------------------

/// Run the codegen and optimizer stage when the `--compile` flag is set.
///
/// When the `--compile` flag is set, the input is a `.opa` AST file
/// produced by the `--parse` stage. The codegen deserializes the AST and
/// compiles it into an object file.
pub fn run(args: &OpcArgs) -> Result<()> {
    if !args.compile {
        return Ok(());
    }
    let json = std::fs::read_to_string(&args.input.input)
        .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", args.input.input))?;
    let ast: AstFile = op_common::from_json(&json)?;
    if ast.file.is_empty() {
        anyhow::bail!(
            "the input .opa file has an empty `file` field; the codegen cannot resolve the \
             standard library without the original source path"
        );
    }
    let (obj, codegen_diags) = compile_source(&ast, args.opt_level, &args.include, &args.features);
    let has_errors = codegen_diags.iter().any(|d| d.severity == Severity::Error);
    if has_errors {
        for d in &codegen_diags {
            d.print(None);
        }
        anyhow::bail!("codegen errors in {}", args.input.input);
    }
    let out = op_common::to_json(&obj)?;
    match &args.output {
        Some(path) => std::fs::write(path, out)?,
        None => println!("{out}"),
    }
    Ok(())
}

/// Compile a source file into an [`ObjectFile`].
pub fn compile_file(
    path: &str,
    target: &str,
    opt_level: u8,
    include_paths: &[String],
    features: &[String],
) -> Result<ObjectFile> {
    compile_file_full(path, target, opt_level, include_paths, features, &[])
}

/// Compile a source file with defined features for `debug_assert!` warnings.
pub fn compile_file_full(
    path: &str,
    target: &str,
    opt_level: u8,
    include_paths: &[String],
    features: &[String],
    defined_features: &[String],
) -> Result<ObjectFile> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", path))?;
    let (ast, parse_diags) =
        parser::parse_source_full(path, &source, target, features, defined_features);
    let has_errors = parse_diags.iter().any(|d| d.severity == Severity::Error);
    if has_errors {
        for d in &parse_diags {
            d.print(None);
        }
        anyhow::bail!("parser errors in {}", path);
    }
    let (obj, codegen_diags) = compile_source(&ast, opt_level, include_paths, features);
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
pub fn compile_source(
    ast: &AstFile,
    opt_level: u8,
    include_paths: &[String],
    features: &[String],
) -> (ObjectFile, Vec<Diagnostic>) {
    let (obj, diags, _tables) = compile_source_with_tables(ast, opt_level, include_paths, features);
    (obj, diags)
}

/// The codegen name tables after a compile, exposed for tests that
/// need to inspect what a `use` declaration imported.
#[derive(Debug, Default)]
pub struct NameTables {
    /// Names of every inline fn known to the codegen, sorted.
    pub inline_fn_names: Vec<String>,
    /// Every constant value keyed by its flat-namespace name.
    pub const_values: HashMap<String, i64>,
}

/// Compile a parsed AST into an [`ObjectFile`], a list of diagnostics,
/// and a snapshot of the codegen name tables.
pub fn compile_source_with_tables(
    ast: &AstFile,
    opt_level: u8,
    include_paths: &[String],
    features: &[String],
) -> (ObjectFile, Vec<Diagnostic>, NameTables) {
    let mut codegen = build_codegen(ast, opt_level, include_paths, features);

    codegen.walk_module(&ast.root);

    let mut inline_fn_names: Vec<String> = codegen.inline_fns.keys().cloned().collect();
    inline_fn_names.sort();

    let tables = NameTables {
        inline_fn_names,
        const_values: codegen.const_values.clone(),
    };

    // Run the peephole optimizer on the sections. The target CPU lets
    // the optimizer skip non-6502-family targets, whose bytes would be
    // decoded as 6502 mnemonics and corrupted.
    let mut sections = codegen.sections;
    crate::optimizer::optimize(&mut sections, opt_level, &codegen.target.cpu);

    let obj = ObjectFile {
        version: 1,
        target: ast.target.clone(),
        sections,
        interrupt_vectors: codegen.interrupt_vectors,
        header: codegen.header,
        pad_byte: codegen.pad_byte,
    };

    (obj, codegen.diags, tables)
}

/// Build a [`Codegen`] for the target of `ast`.
fn build_codegen(
    ast: &AstFile,
    opt_level: u8,
    include_paths: &[String],
    features: &[String],
) -> Codegen {
    let triplet = TargetTriplet::parse(&ast.target).unwrap_or(TargetTriplet {
        cpu: String::new(),
        manufacturer: String::new(),
        machine: String::new(),
        variant: String::new(),
    });

    let encoding_table = get_full_encoding_table(&triplet.cpu);

    Codegen {
        target: triplet,
        opt_level,
        current_fn: String::new(),
        encoding_table,
        sections: Vec::new(),
        current_section: None,
        inline_fns: HashMap::new(),
        const_values: HashMap::new(),
        const_arrays: HashMap::new(),
        struct_consts: HashMap::new(),
        collected_vars: Vec::new(),
        collected_consts: Vec::new(),
        symbol_types: HashMap::new(),
        module_cache: ModuleCache::default(),
        module_path: Vec::new(),
        enum_variants: HashMap::new(),
        use_aliases: HashMap::new(),
        include_paths: include_paths.to_vec(),
        features: features.to_vec(),
        current_module_dir: None,
        label_counter: 0,
        interrupt_vectors: Vec::new(),
        header: None,
        pad_byte: 0x00,
        source_dir: std::path::Path::new(&ast.file)
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .to_path_buf(),
        placed_items: std::collections::HashSet::new(),
        crash_handler_symbol: "__op_default_crash_handler".to_string(),
        struct_sizes: HashMap::new(),
        pending_locate: (None, None),
        fn_locate_pins: HashMap::new(),
        noreturn_fns: std::collections::HashSet::new(),
        collected_locates: HashMap::new(),
        diags: Vec::new(),
    }
}

// --- Module cache -----------------------------------------------------------

/// Parsed std modules keyed by absolute file path.
///
/// Each file is parsed once. Later references to the same file reuse
/// the cached [`Module`], which prevents duplicate parses and
/// duplicate diagnostics.
#[derive(Default)]
struct ModuleCache {
    modules: HashMap<std::path::PathBuf, CachedModule>,
}

/// A parsed module plus the directory of the file it was loaded
/// from. The directory lets placement macros inside a std module
/// (such as `#[locate(file = ...)]`) resolve paths relative to the std file
/// instead of the root source file.
struct CachedModule {
    module: Module,
    dir: std::path::PathBuf,
}

impl ModuleCache {
    /// Load the module at `path`, parsing the file on a cache miss.
    ///
    /// cfg evaluation happens during parsing: the parser drops items
    /// whose `#[cfg]` predicate does not match `target`.
    fn load_module(
        &mut self,
        path: &std::path::Path,
        target: &TargetTriplet,
        features: &[String],
    ) -> Result<Module> {
        let key = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        if let Some(entry) = self.modules.get(&key) {
            return Ok(entry.module.clone());
        }
        let path_str = key.to_string_lossy().to_string();
        let source = std::fs::read_to_string(&key)
            .map_err(|e| anyhow::anyhow!("failed to read {path_str}: {e}"))?;
        let (ast, diags) = parser::parse_source(&path_str, &source, &target.as_str(), features);
        let has_errors = diags.iter().any(|d| d.severity == Severity::Error);
        if has_errors {
            for d in &diags {
                d.print(None);
            }
            anyhow::bail!("parser errors in {path_str}");
        }
        let module = ast.root;
        let dir = key
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| key.clone());
        self.modules.insert(
            key,
            CachedModule {
                module: module.clone(),
                dir,
            },
        );
        Ok(module)
    }

    /// Return the directory of the file a cached module was loaded
    /// from.
    fn dir_of(&self, path: &std::path::Path) -> Option<&std::path::PathBuf> {
        let key = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        self.modules.get(&key).map(|entry| &entry.dir)
    }
}

// --- Inline functions -------------------------------------------------------

/// A placement root identified during the placement pass.
struct PlacementRoot {
    name: String,
    section_idx: Option<usize>,
}

/// A function stored for call-site expansion or subroutine linkage.
/// `is_inline` is true for `inline fn` declarations (and for std inline
/// fns imported via `use`); their bodies are substituted at each call
/// site. `is_inline` is false for non-inline `fn` declarations; calls to
/// them emit a `jsr` plus an `Abs16` relocation against the fn name, and
/// the body is placed exactly once.
#[derive(Clone)]
struct InlineFn {
    params: Vec<String>,
    body: Vec<FnStmt>,
    is_inline: bool,
}

// --- Codegen struct ---------------------------------------------------------

struct Codegen {
    target: TargetTriplet,
    opt_level: u8,
    /// Name of the fn whose body is currently being compiled. The
    /// flow-control diagnostics name this fn. Inline fn bodies keep
    /// the name of the fn whose call site is being expanded.
    current_fn: String,
    encoding_table: Vec<&'static crate::encoding::EncodingEntry>,
    sections: Vec<Section>,
    current_section: Option<usize>,
    inline_fns: HashMap<String, InlineFn>,
    const_values: HashMap<String, i64>,
    /// Byte arrays for `const [u8; N] = [...]` declarations, keyed by name.
    const_arrays: HashMap<String, Vec<u8>>,
    /// Struct const field expressions, keyed by const name.
    /// Each entry is a list of (field_name, field_expr) pairs.
    struct_consts: HashMap<String, Vec<(String, Expr)>>,
    /// All var declarations collected from all modules (including
    /// sub-modules), stored for placement in the first RAM section.
    /// Each entry is (name, ty, addr_binding, init).
    collected_vars: Vec<(String, Type, Option<Expr>, Option<InitValue>)>,
    /// All const declarations collected from all modules (including
    /// sub-modules), stored for placement in the first ROM section.
    /// Each entry is (name, ty, value, evaluated_value).
    collected_consts: Vec<(String, Type, Expr, Option<i64>)>,
    /// `#[locate(...)]` pins captured from imported std consts, keyed by
    /// const name: (addr, file). Consumed by the placement pass when the
    /// const is placed.
    collected_locates: HashMap<String, (Option<u32>, Option<String>)>,
    /// Types of top-level const and var declarations, keyed by name.
    /// Populated during the collect pass before values are evaluated,
    /// so that `len!` and `sizeof!` of a later declaration resolve.
    symbol_types: HashMap<String, Type>,
    /// Parsed std modules keyed by absolute file path.
    module_cache: ModuleCache,
    /// Current module path stack. Empty at the crate root. A name is
    /// pushed when the codegen enters a module and popped when it
    /// leaves.
    module_path: Vec<String>,
    /// Enum variant values keyed by `EnumName::VariantName`.
    enum_variants: HashMap<String, i64>,
    /// Module paths bound by `use ... as alias` imports.
    use_aliases: HashMap<String, Vec<String>>,
    /// Include search paths from the CLI. The first directory that
    /// contains a `lib.op` file is the std crate root.
    include_paths: Vec<String>,
    /// Feature flags from the CLI. Used when parsing std modules.
    features: Vec<String>,
    label_counter: u32,
    interrupt_vectors: Vec<op_ir::InterruptVector>,
    header: Option<op_ir::HeaderFields>,
    pad_byte: u8,
    /// Directory of the std module file whose items are currently
    /// being walked, if any. `None` while walking the root source
    /// file. `#[locate(file = ...)]` and `locate_str!` inside std modules
    /// resolve paths against this directory.
    current_module_dir: Option<std::path::PathBuf>,
    /// Directory of the root source file. Used to resolve
    /// and `locate_str!` paths.
    source_dir: std::path::PathBuf,
    /// Names of top-level fns, consts, and vars that the placement pass
    /// has already placed into a section. The compile walk skips these
    /// to avoid double emission.
    placed_items: std::collections::HashSet<String>,
    /// The crash handler symbol name. Set during the collect pass by
    /// scanning for `#[crash_handler]` on fn declarations. Defaults to
    /// `__op_default_crash_handler`.
    crash_handler_symbol: String,
    /// Struct sizes keyed by struct name. Populated during the collect
    /// pass from StructDecl items. Used by `self.type_size` to compute
    /// the size of struct types (which the standalone `type_size` cannot
    /// do because it has no access to struct definitions).
    struct_sizes: HashMap<String, usize>,
    /// Pending `#[locate(...)]` attribute for the const the placement
    /// pass is about to place: (addr, file). Set from the const's
    /// declarations during the placement pass and consumed by
    /// `place_const`.
    pending_locate: (Option<u32>, Option<String>),
    /// `noreturn` fns keyed by name. The placement pass compiles via
    /// `compile_fn(..., is_noreturn)`; this map carries the flag from the
    /// declaration to the compile call.
    noreturn_fns: std::collections::HashSet<String>,
    /// `#[locate(addr = ...)]` pins for in-block fns keyed by fn name.
    /// The pin moves the fn body start to the absolute address. Consumed
    /// in `place_fn_tree` right before the fn compiles.
    fn_locate_pins: std::collections::HashMap<String, u32>,
    diags: Vec<Diagnostic>,
}

// --- Module walking ---------------------------------------------------------

impl Codegen {
    fn walk_module(&mut self, module: &Module) {
        // Collect constants, enum variants, and imports first so that
        // fn bodies can reference them regardless of the order the
        // items appear in the file.
        self.collect_module_items(&module.items);
        // Create sections from block attributes before placement so
        // the placer can append fn bodies and data into them.
        self.create_sections(&module.items);
        // Place reachable fns and top-level data into sections, then
        // emit dead-code warnings for unreachable items.
        self.placement_pass(&module.items);
        // Walk items for codegen (section blocks, locate_str!, etc.).
        // Top-level fns/consts/vars placed by the placer are skipped.
        for item in &module.items {
            self.walk_item(item);
        }
    }

    /// Collect constant and enum variant values from `items` without
    /// compiling any fn bodies. Also resolves `use` declarations and
    /// recurses into sub-modules and attribute blocks, so every name
    /// is bound before the first fn body is compiled.
    fn collect_module_items(&mut self, items: &[Item]) {
        // Register the types of all const and var declarations first,
        // so that `len!` and `sizeof!` of a later declaration resolve
        // when an earlier declaration's value is evaluated.
        for item in items {
            self.register_type(item);
        }
        for item in items {
            self.collect_item(item);
        }
        // Scan for #[crash_handler] on fn declarations.
        for item in items {
            if let Item::FnDecl {
                name, attributes, ..
            } = item
            {
                for attr in attributes {
                    if attr.path == "crash_handler" {
                        self.crash_handler_symbol = name.clone();
                        break;
                    }
                }
            }
        }
    }

    /// Compute the size of a type, consulting `struct_sizes` for
    /// struct types that the standalone `type_size` cannot handle.
    fn type_size(&self, ty: &Type) -> usize {
        match ty {
            Type::Named { name } => {
                if let Some(size) = self.struct_sizes.get(name) {
                    *size
                } else {
                    type_size(ty)
                }
            }
            Type::Array { element, size } => {
                let elem_size = self.type_size(element);
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

    /// Record the type of a top-level const or var declaration in
    /// `symbol_types`. Also register struct sizes from StructDecl items.
    fn register_type(&mut self, item: &Item) {
        match item {
            Item::ConstDecl { name, ty, .. } | Item::VarDecl { name, ty, .. } => {
                self.symbol_types.insert(name.clone(), ty.clone());
            }
            Item::StructDecl { name, fields, .. } => {
                let size: usize = fields
                    .iter()
                    .map(|f| {
                        let base = type_size(&f.ty);
                        if let Some(dim) = &f.array_dim {
                            if let Some(d) = eval_const_expr_simple(dim) {
                                base * d as usize
                            } else {
                                base
                            }
                        } else {
                            base
                        }
                    })
                    .sum();
                self.struct_sizes.insert(name.clone(), size);
            }
            _ => {}
        }
    }

    fn collect_item(&mut self, item: &Item) {
        match item {
            Item::ConstDecl {
                name,
                value,
                evaluated_value,
                ..
            } => {
                match value {
                    // Array literal const: collect byte values.
                    Expr::ArrayLit { elements } => {
                        let mut bytes = Vec::new();
                        for elem in elements {
                            if let Some(val) =
                                eval_expr(elem, &self.const_values, &self.symbol_types)
                            {
                                bytes.push(val as u8);
                            } else {
                                bytes.push(0);
                            }
                        }
                        self.const_arrays.insert(name.clone(), bytes);
                        // Store the length in const_values for len!/sizeof!.
                        self.const_values
                            .insert(name.clone(), elements.len() as i64);
                    }
                    // Struct literal const: store field expressions.
                    Expr::StructLit { fields, .. } => {
                        let field_exprs: Vec<(String, Expr)> = fields.clone();
                        // Evaluate scalar fields into const_values as
                        // CONST::field = value. Skip pointer fields
                        // that reference array consts — their value
                        // is an address (relocation), not a length.
                        for (field_name, field_expr) in fields {
                            let is_array_ref = matches!(
                                field_expr,
                                Expr::Ident { name } if self.const_arrays.contains_key(name)
                            );
                            if !is_array_ref {
                                if let Some(val) =
                                    eval_expr(field_expr, &self.const_values, &self.symbol_types)
                                {
                                    self.const_values
                                        .insert(format!("{}::{}", name, field_name), val);
                                }
                            }
                        }
                        self.struct_consts.insert(name.clone(), field_exprs);
                    }
                    // Scalar const: use evaluated value.
                    _ => {
                        let val = (*evaluated_value)
                            .or_else(|| eval_expr(value, &self.const_values, &self.symbol_types));
                        if let Some(val) = val {
                            self.const_values.insert(name.clone(), val);
                        }
                    }
                }
            }
            Item::EnumDecl { name, variants, .. } => {
                self.collect_enum(name, variants, false);
            }
            Item::UseDecl { trees, .. } => {
                self.resolve_use_decl(trees);
            }
            Item::ModDecl {
                name,
                resolved,
                body,
                ..
            } => {
                self.module_path.push(name.clone());
                if let Some(sub_module) = resolved {
                    self.collect_module_items(&sub_module.items);
                } else if let Some(items) = body {
                    self.collect_module_items(items);
                }
                self.module_path.pop();
            }
            Item::BlockAttribute { items, .. } => {
                self.collect_module_items(items);
            }
            Item::FnDecl {
                name,
                is_noreturn,
                body,
                ..
            } => {
                if *is_noreturn {
                    self.noreturn_fns.insert(name.clone());
                }
                self.inline_fns.insert(
                    name.clone(),
                    InlineFn {
                        params: Vec::new(),
                        body: body.clone(),
                        is_inline: false,
                    },
                );
            }
            Item::InlineFnDecl {
                name, params, body, ..
            } => {
                self.inline_fns.insert(
                    name.clone(),
                    InlineFn {
                        params: params.clone(),
                        body: body.clone(),
                        is_inline: true,
                    },
                );
            }
            Item::VarDecl {
                name,
                ty,
                addr_binding,
                init,
                ..
            } => {
                self.collected_vars.push((
                    name.clone(),
                    ty.clone(),
                    addr_binding.clone(),
                    init.clone(),
                ));
            }
            _ => {}
        }
    }

    /// Create sections from `#[rom]`, `#[ram]`, and `#[chr]` block
    /// attributes without walking their items. This runs before the
    /// placement pass so the placer can append fn bodies and data into
    /// the already-created sections.
    fn create_sections(&mut self, items: &[Item]) {
        for item in items {
            if let Item::BlockAttribute { attr, .. } = item {
                // Capture header fields from #[ines], #[gb], etc. before
                // the placement pass so the mapper guard can use them.
                self.capture_header(attr);
                self.create_section_from_attr(attr);
            }
        }
    }

    /// Capture header fields from a standalone attribute like `#[ines(...)]`
    /// into `self.header`.
    fn capture_header(&mut self, attr: &Attribute) {
        match attr.path.as_str() {
            "ines" | "gb" | "lnx" | "snes" | "sega" | "sms" | "a78" | "crt" => {
                let format = attr.path.as_str();
                let fields: Vec<(String, String)> = attr
                    .args
                    .iter()
                    .map(|a| (a.name.clone(), a.value.trim_matches('"').to_string()))
                    .collect();
                self.header = Some(op_ir::HeaderFields {
                    format: format.to_string(),
                    fields,
                });
            }
            _ => {}
        }
    }

    /// Return true when the source declares a `#[gb(...)]` header
    /// attribute. The SM83 0x150 reservation and the emit-time header
    /// writes both depend on it.
    fn header_is_gb(&self) -> bool {
        matches!(&self.header, Some(h) if h.format == "gb")
    }

    /// Return true when the source declares a `#[crt(...)]` header
    /// attribute. The W65C02 (Commander X16) cartridge signature
    /// reservation and the emit-time header writes both depend on it.
    fn header_is_crt(&self) -> bool {
        matches!(&self.header, Some(h) if h.format == "crt")
    }

    /// Create a single section from a block attribute (rom/ram/chr).
    fn create_section_from_attr(&mut self, attr: &Attribute) {
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
        // Don't create duplicate sections (handle_block_attribute may
        // also try to create one during the walk).
        if self.sections.iter().any(|s| s.name == name) {
            return;
        }
        self.sections.push(Section {
            name,
            kind,
            org,
            bank,
            maxsize,
            symbols: Vec::new(),
            relocations: Vec::new(),
            data: Vec::new(),
        });
    }

    /// Placement pass: gather roots, build a dependency tree, place
    /// reachable fns and top-level data into sections, and emit
    /// dead-code warnings. Runs after `collect_module_items` and
    /// `create_sections`, before the compile walk.
    fn placement_pass(&mut self, items: &[Item]) {
        // If there are no sections, the placement pass is a no-op
        // (the source has no rom/ram/chr blocks).
        if self.sections.is_empty() {
            return;
        }

        // Gather roots in declaration order.
        let roots = self.gather_roots(items);

        // A `#[locate]` pin on any const is a placement directive even
        // without fn roots: data-only ROM images (a boot ROM, a font
        // bank) must still place.
        let has_pinned_const = Self::items_have_pinned_const(items)
            || self
                .collected_locates
                .values()
                .any(|(addr, _)| addr.is_some());
        if roots.is_empty() && !has_pinned_const {
            return;
        }

        // Build a set of all top-level fn names (non-inline) and
        // inline fn names for dead-code checking.
        let mut all_non_inline_fns: Vec<String> = Vec::new();
        let mut all_inline_fns: Vec<String> = Vec::new();
        let mut all_consts: Vec<String> = Vec::new();
        let mut all_vars: Vec<String> = Vec::new();
        let mut all_enums: Vec<String> = Vec::new();
        for item in items {
            match item {
                Item::FnDecl { name, .. } => all_non_inline_fns.push(name.clone()),
                Item::InlineFnDecl { name, .. } => all_inline_fns.push(name.clone()),
                Item::ConstDecl { name, .. } => all_consts.push(name.clone()),
                Item::VarDecl { name, .. } => all_vars.push(name.clone()),
                Item::EnumDecl { name, .. } => all_enums.push(name.clone()),
                _ => {}
            }
        }

        // Track which items have been placed/referenced.
        let mut placed_fns: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut called_inline_fns: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        let mut referenced_data: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        let mut referenced_enums: std::collections::HashSet<String> =
            std::collections::HashSet::new();

        // Find the first ROM section index (for unpinned interrupt roots).
        let first_rom = self
            .sections
            .iter()
            .position(|s| s.kind == SectionKind::Rom);
        // Find the first RAM section index (for top-level vars).
        let first_ram = self
            .sections
            .iter()
            .position(|s| s.kind == SectionKind::Ram);

        // SM83 (Game Boy): reserve the first 0x150 bytes of the bank-0 ROM
        // section (org 0x0000) for the interrupt vectors (0x0040-0x0100) and
        // the cartridge header (0x0104-0x014F). Code then starts at 0x0150,
        // so the reset handler is not clobbered by the header written at
        // emit time. The output stage (emit_gb) zeros this region, writes the
        // header, and patches the reset vector.
        // The reservation runs only when the source declares a `#[gb]`
        // header. A raw-format program (for example, a boot ROM) keeps full
        // control of $0000-$014F.
        if self.target.cpu == "sm83" && self.header_is_gb() {
            if let Some(idx) = first_rom {
                let section = &mut self.sections[idx];
                if section.org == 0 && section.data.len() < 0x0150 {
                    section.data.resize(0x0150, 0);
                }
            }
        }

        // W65C02 (Commander X16): reserve the first 4 bytes of the bank-32
        // ROM section (org 0xC000) for the cartridge boot signature. The
        // X16 KERNAL checks $C000-$C003 of bank 32 for the "CX16" signature
        // and enters the cartridge at $C004, so code must start at offset 4
        // and must not clobber the signature bytes. The output stage
        // (emit_crt) writes the signature into this gap at emit time. The
        // reservation runs only when the source declares a `#[crt]` header;
        // a PRG program keeps full control of the section start.
        if self.target.cpu == "w65c02" && self.header_is_crt() {
            if let Some(idx) = self
                .sections
                .iter()
                .position(|s| s.kind == SectionKind::Rom && s.bank == 32 && s.org == 0xC000)
            {
                let section = &mut self.sections[idx];
                if section.data.len() < 4 {
                    section.data.resize(4, 0);
                }
            }
        }

        // Walk items in declaration order. Fns with a pin place at their
        // pin; pinned consts emit at their pin; this lets data sit between
        // two code regions exactly as the source declares. Unpinned fn
        // roots ALSO place here in declaration order, so plain code lands
        // ahead of later pinned items in source order. Unpinned consts
        // defer to the loop after this walk (reference-driven).
        for item in items {
            match item {
                Item::FnDecl { name, .. } => {
                    if placed_fns.contains(name) {
                        continue;
                    }
                    let pinned = self.fn_locate_pins.contains_key(name);
                    let is_root = roots.iter().any(|r| &r.name == name);
                    if pinned || is_root {
                        let root = roots.iter().find(|r| &r.name == name);
                        let idx = root.and_then(|r| r.section_idx).or(first_rom);
                        if let Some(idx) = idx {
                            self.place_fn_tree(
                                name,
                                idx,
                                &mut placed_fns,
                                &mut called_inline_fns,
                                &mut referenced_data,
                                &mut referenced_enums,
                            );
                        }
                    }
                }
                Item::ConstDecl {
                    name,
                    ty,
                    value,
                    evaluated_value,
                    attributes,
                    ..
                } => {
                    if self.placed_items.contains(name) {
                        continue;
                    }
                    let (locate_addr, locate_file) =
                        Self::find_locate_attr(attributes).unwrap_or((None, None));
                    if locate_addr.is_none() && locate_file.is_none() {
                        continue;
                    }
                    if let Some(rom_idx) = first_rom {
                        if let (Some(addr), Some(file)) = (locate_addr, locate_file.clone()) {
                            self.place_const_file(name, addr, &file, rom_idx);
                            self.placed_items.insert(name.clone());
                            continue;
                        }
                        if locate_addr.is_some() || locate_file.is_some() {
                            self.pending_locate = (locate_addr, locate_file.clone());
                        }
                        self.place_const(name, ty, value, *evaluated_value, rom_idx);
                        self.pending_locate = (None, None);
                        self.placed_items.insert(name.clone());
                    }
                }
                Item::BlockAttribute {
                    items: block_items,
                    attr,
                    ..
                } if attr.path == "rom" || attr.path == "chr" => {
                    let kind = if attr.path == "rom" {
                        SectionKind::Rom
                    } else {
                        SectionKind::Chr
                    };
                    let bank = get_attr_u32(attr, "bank").unwrap_or(0);
                    let sec_name = format!("{}_bank{}", kind_name(kind), bank);
                    let sec_idx = self.sections.iter().position(|s| s.name == sec_name);
                    for bi in block_items {
                        match bi {
                            Item::FnDecl { name, .. } => {
                                if placed_fns.contains(name) {
                                    continue;
                                }
                                let pinned = self.fn_locate_pins.contains_key(name);
                                let is_root = roots.iter().any(|r| &r.name == name);
                                if pinned || is_root {
                                    if let Some(idx) = sec_idx {
                                        self.place_fn_tree(
                                            name,
                                            idx,
                                            &mut placed_fns,
                                            &mut called_inline_fns,
                                            &mut referenced_data,
                                            &mut referenced_enums,
                                        );
                                    }
                                }
                            }
                            Item::ConstDecl {
                                name,
                                ty,
                                value,
                                evaluated_value,
                                attributes,
                                ..
                            } => {
                                if self.placed_items.contains(name) {
                                    continue;
                                }
                                let (locate_addr, locate_file) =
                                    Self::find_locate_attr(attributes).unwrap_or((None, None));
                                if locate_addr.is_none() && locate_file.is_none() {
                                    continue;
                                }
                                if let Some(rom_idx) = sec_idx.or(first_rom) {
                                    if let (Some(addr), Some(file)) =
                                        (locate_addr, locate_file.clone())
                                    {
                                        self.place_const_file(name, addr, &file, rom_idx);
                                        self.placed_items.insert(name.clone());
                                        continue;
                                    }
                                    if locate_addr.is_some() || locate_file.is_some() {
                                        self.pending_locate = (locate_addr, locate_file.clone());
                                    }
                                    self.place_const(name, ty, value, *evaluated_value, rom_idx);
                                    self.pending_locate = (None, None);
                                    self.placed_items.insert(name.clone());
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        // Place top-level vars in the first RAM section (never duplicated).
        if let Some(ram_idx) = first_ram {
            for item in items {
                if let Item::VarDecl {
                    name,
                    ty,
                    addr_binding,
                    init,
                    ..
                } = item
                {
                    if !self.placed_items.contains(name) {
                        self.current_section = Some(ram_idx);
                        self.alloc_variable(name, ty, addr_binding, init);
                        self.current_section = None;
                        self.placed_items.insert(name.clone());
                    }
                }
            }

            // Place vars collected from sub-modules (e.g. std::font::ram).
            let collected = self.collected_vars.clone();
            for (name, ty, addr_binding, init) in &collected {
                if !self.placed_items.contains(name) {
                    self.current_section = Some(ram_idx);
                    self.alloc_variable(name, ty, addr_binding, init);
                    self.current_section = None;
                    self.placed_items.insert(name.clone());
                }
            }
        }

        // Place top-level consts that were referenced by live code.
        // Process in first-reference order (which is the order they
        // appear in referenced_data, built during DFS). Consts declared
        // inside `#[rom]` blocks participate the same way; the block
        // declares their ROM section.
        let mut const_items: Vec<(&Item, Option<usize>)> = Vec::new();
        for item in items {
            match item {
                Item::ConstDecl { .. } => const_items.push((item, None)),
                Item::BlockAttribute {
                    attr,
                    items: block_items,
                } if attr.path == "rom" || attr.path == "chr" => {
                    let kind = if attr.path == "rom" {
                        SectionKind::Rom
                    } else {
                        SectionKind::Chr
                    };
                    let bank = get_attr_u32(attr, "bank").unwrap_or(0);
                    let name = format!("{}_bank{}", kind_name(kind), bank);
                    let idx = self.sections.iter().position(|s| s.name == name);
                    for bi in block_items {
                        if let Item::ConstDecl { .. } = bi {
                            const_items.push((bi, idx));
                        }
                    }
                }
                _ => {}
            }
        }
        for (item, block_section) in const_items {
            if let Item::ConstDecl {
                name,
                ty,
                value,
                evaluated_value,
                attributes,
                ..
            } = item
            {
                // A `#[locate]` pin is a placement directive: the const
                // places even when no live code references it. Without a
                // pin, the refered-only rule applies as before.
                let pinned = Self::find_locate_attr(attributes).is_some();
                if (pinned || referenced_data.contains(name)) && !self.placed_items.contains(name) {
                    if let Some(rom_idx) = block_section.or(first_rom) {
                        // #[locate(addr = ...)] pins the const at an
                        // absolute address within its block. With a
                        // `file` argument, the file bytes emit at the
                        // pinned address instead of the const's value.
                        let (locate_addr, locate_file) =
                            Self::find_locate_attr(attributes).unwrap_or((None, None));
                        if let (Some(addr), Some(file)) = (locate_addr, locate_file.clone()) {
                            self.place_const_file(name, addr, &file, rom_idx);
                            self.placed_items.insert(name.clone());
                            continue;
                        }
                        if locate_addr.is_some() || locate_file.is_some() {
                            self.pending_locate = (locate_addr, locate_file.clone());
                        }
                        self.place_const(name, ty, value, *evaluated_value, rom_idx);
                        self.pending_locate = (None, None);
                        self.placed_items.insert(name.clone());
                    }
                }
            }
        }

        // Place consts collected from sub-modules (e.g. std font data)
        // that were referenced by live code. A collected const with a
        // `#[locate]` pin places even without a reference: the pin is a
        // placement directive.
        let collected = self.collected_consts.clone();
        for (name, ty, value, evaluated_value) in &collected {
            let (locate_addr, locate_file) = self
                .collected_locates
                .get(name)
                .cloned()
                .unwrap_or((None, None));
            let pinned = self.collected_locates.contains_key(name);
            if (referenced_data.contains(name) || pinned) && !self.placed_items.contains(name) {
                if let Some(rom_idx) = first_rom {
                    if let (Some(addr), Some(file)) = (locate_addr, locate_file.clone()) {
                        self.place_const_file(name, addr, &file, rom_idx);
                        self.placed_items.insert(name.clone());
                        continue;
                    }
                    if let Some(addr) = locate_addr {
                        self.pending_locate = (Some(addr), locate_file);
                    }
                    self.place_const(name, ty, value, *evaluated_value, rom_idx);
                    self.pending_locate = (None, None);
                    self.placed_items.insert(name.clone());
                }
            }
        }

        // Dead-code warnings.
        for name in &all_non_inline_fns {
            if !placed_fns.contains(name) {
                self.warning(
                    306,
                    format!("function `{name}` is never called from any root; dead code"),
                );
            }
        }
        for name in &all_inline_fns {
            if !called_inline_fns.contains(name) {
                self.warning(
                    306,
                    format!("inline function `{name}` is never called; dead code"),
                );
            }
        }
        for name in &all_consts {
            if !referenced_data.contains(name) {
                self.warning(
                    306,
                    format!("constant `{name}` is never referenced; dead code"),
                );
            }
        }
        for name in &all_vars {
            if !referenced_data.contains(name) {
                self.warning(
                    306,
                    format!("variable `{name}` is never referenced; dead code"),
                );
            }
        }
        for name in &all_enums {
            if !referenced_enums.contains(name) {
                self.warning(
                    306,
                    format!("enum `{name}` has no variant referenced; dead code"),
                );
            }
        }
    }

    /// Gather placement roots from top-level items in declaration order.
    #[allow(clippy::collapsible_if, clippy::collapsible_match)]
    /// Return true when any const declaration among the items (top
    /// level or inside a `#[rom]` block) carries a `#[locate(...)]`
    /// attribute.
    fn items_have_pinned_const(items: &[Item]) -> bool {
        for item in items {
            match item {
                Item::ConstDecl { attributes, .. } => {
                    if attributes.iter().any(|a| a.path == "locate") {
                        return true;
                    }
                }
                Item::BlockAttribute {
                    attr,
                    items: block_items,
                } => {
                    if attr.path == "rom"
                        && block_items.iter().any(|bi| {
                            matches!(bi, Item::ConstDecl { attributes, .. }
                                if attributes.iter().any(|a| a.path == "locate"))
                        })
                    {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    fn gather_roots(&mut self, items: &[Item]) -> Vec<PlacementRoot> {
        let mut roots = Vec::new();
        let first_rom = self
            .sections
            .iter()
            .position(|s| s.kind == SectionKind::Rom);

        for item in items {
            match item {
                Item::FnDecl {
                    name, attributes, ..
                } => {
                    // #[interrupt] on a fn definition makes it a root.
                    let has_interrupt = attributes.iter().any(|a| a.path == "interrupt");
                    if has_interrupt {
                        roots.push(PlacementRoot {
                            name: name.clone(),
                            section_idx: first_rom,
                        });
                    }
                }
                Item::BlockAttribute {
                    attr,
                    items: block_items,
                } if attr.path == "rom" => {
                    let section_idx = self.sections.iter().position(|s| {
                        s.kind == SectionKind::Rom
                            && s.bank == get_attr_u32(attr, "bank").unwrap_or(0)
                    });
                    for block_item in block_items {
                        if let Item::FnDecl {
                            name, attributes, ..
                        } = block_item
                        {
                            // In-block fn is a root (placed at its
                            // position in the block). A
                            // `#[locate(addr = ...)]` pin moves the
                            // body start to that absolute address.
                            let locate =
                                Self::find_locate_attr(attributes).and_then(|(addr, _)| addr);
                            if let Some(addr) = locate {
                                self.fn_locate_pins.insert(name.clone(), addr);
                            }
                            let _has_interrupt = attributes.iter().any(|a| a.path == "interrupt");
                            roots.push(PlacementRoot {
                                name: name.clone(),
                                section_idx,
                            });
                        }
                    }
                }
                _ => {}
            }
        }
        roots
    }

    /// Place a fn and its transitive callees into a section using DFS.
    fn place_fn_tree(
        &mut self,
        fn_name: &str,
        section_idx: usize,
        placed_fns: &mut std::collections::HashSet<String>,
        called_inline_fns: &mut std::collections::HashSet<String>,
        referenced_data: &mut std::collections::HashSet<String>,
        referenced_enums: &mut std::collections::HashSet<String>,
    ) {
        if placed_fns.contains(fn_name) {
            return;
        }
        placed_fns.insert(fn_name.to_string());

        // Get the fn body from inline_fns (where FnDecl stores it).
        let inline_fn = match self.inline_fns.get(fn_name).cloned() {
            Some(f) => f,
            None => return,
        };

        // Walk the body to find callees and data references before
        // compiling (so we can place callees first, then compile this
        // fn which will emit jsr relocations that resolve).
        let (callees, data_refs, enum_refs, inline_calls) =
            self.analyze_body(&inline_fn.body, &inline_fn.params, &inline_fn.is_inline);

        // Record inline fn calls for dead-code detection.
        for name in &inline_calls {
            called_inline_fns.insert(name.clone());
        }
        // Record data references.
        for name in &data_refs {
            referenced_data.insert(name.clone());
        }
        // Record enum references.
        for name in &enum_refs {
            referenced_enums.insert(name.clone());
        }

        // Compile this fn into the section first (roots appear before
        // their callees in the block). A `#[locate(addr = ...)]` pin on
        // the fn moves its body start to the absolute address: the gap
        // up to the pin fills with the pad byte.
        self.current_section = Some(section_idx);
        if let Some(&addr) = self.fn_locate_pins.get(fn_name) {
            if !self.locate_to_addr(addr, fn_name) {
                self.current_section = None;
                self.placed_items.insert(fn_name.to_string());
                return;
            }
        }
        let is_noreturn = self.noreturn_fns.contains(fn_name);
        self.compile_fn(fn_name, &inline_fn.body, is_noreturn);
        self.current_section = None;
        self.fn_locate_pins.remove(fn_name);
        self.placed_items.insert(fn_name.to_string());

        // Then place callees (DFS, first-call order).
        for callee in &callees {
            if !placed_fns.contains(callee) {
                self.place_fn_tree(
                    callee,
                    section_idx,
                    placed_fns,
                    called_inline_fns,
                    referenced_data,
                    referenced_enums,
                );
            }
        }
    }

    /// Analyze a fn body to find callees, data references, enum
    /// references, and inline fn calls. Expands inline fn bodies to
    /// find transitive references.
    fn analyze_body(
        &self,
        body: &[FnStmt],
        params: &[String],
        is_inline: &bool,
    ) -> (Vec<String>, Vec<String>, Vec<String>, Vec<String>) {
        let _ = is_inline;
        let mut callees = Vec::new();
        let mut data_refs = Vec::new();
        let mut enum_refs = Vec::new();
        let mut inline_calls = Vec::new();
        self.analyze_stmts(
            body,
            params,
            &mut callees,
            &mut data_refs,
            &mut enum_refs,
            &mut inline_calls,
        );
        (callees, data_refs, enum_refs, inline_calls)
    }

    /// Recursively walk statements to find references.
    fn analyze_stmts(
        &self,
        stmts: &[FnStmt],
        params: &[String],
        callees: &mut Vec<String>,
        data_refs: &mut Vec<String>,
        enum_refs: &mut Vec<String>,
        inline_calls: &mut Vec<String>,
    ) {
        for stmt in stmts {
            self.analyze_stmt(stmt, params, callees, data_refs, enum_refs, inline_calls);
        }
    }

    fn analyze_stmt(
        &self,
        stmt: &FnStmt,
        params: &[String],
        callees: &mut Vec<String>,
        data_refs: &mut Vec<String>,
        enum_refs: &mut Vec<String>,
        inline_calls: &mut Vec<String>,
    ) {
        match stmt {
            FnStmt::AsmStmt { operands, .. } => {
                for operand in operands {
                    self.analyze_operand(operand, data_refs, enum_refs, inline_calls, callees);
                }
            }
            FnStmt::FnCall { name, args } => {
                // Check if this is an inline or non-inline fn.
                if let Some(inline_fn) = self.inline_fns.get(name) {
                    if inline_fn.is_inline {
                        inline_calls.push(name.clone());
                        // Expand: analyze the inline fn's body with
                        // substituted params.
                        let substituted =
                            self.substitute_params(&inline_fn.body, &inline_fn.params, args);
                        self.analyze_stmts(
                            &substituted,
                            &inline_fn.params,
                            callees,
                            data_refs,
                            enum_refs,
                            inline_calls,
                        );
                    } else {
                        // Non-inline fn call → callee edge.
                        if !callees.contains(name) {
                            callees.push(name.clone());
                        }
                    }
                } else {
                    // Unknown fn (stdlib or external) — not a callee for placement.
                }
                // Analyze args for data refs.
                for arg in args {
                    self.analyze_expr(arg, data_refs, enum_refs, inline_calls, callees);
                }
            }
            FnStmt::Label { stmt, .. } => {
                self.analyze_stmt(stmt, params, callees, data_refs, enum_refs, inline_calls);
            }
            FnStmt::IfStmt {
                then_block,
                else_block,
                ..
            } => {
                self.analyze_stmts(
                    then_block,
                    params,
                    callees,
                    data_refs,
                    enum_refs,
                    inline_calls,
                );
                if let Some(else_stmts) = else_block {
                    self.analyze_stmts(
                        else_stmts,
                        params,
                        callees,
                        data_refs,
                        enum_refs,
                        inline_calls,
                    );
                }
            }
            FnStmt::WhileStmt { body, .. }
            | FnStmt::DoWhileStmt { body, .. }
            | FnStmt::LoopStmt { body } => {
                self.analyze_stmts(body, params, callees, data_refs, enum_refs, inline_calls);
            }
            FnStmt::SwitchStmt { cases, .. } => {
                for case in cases {
                    let (SwitchCase::Case { body, .. } | SwitchCase::Default { body }) = case;
                    self.analyze_stmts(body, params, callees, data_refs, enum_refs, inline_calls);
                }
            }
            FnStmt::ReturnStmt | FnStmt::VarDeclStmt { .. } => {}
        }
    }

    #[allow(clippy::collapsible_if)]
    fn analyze_operand(
        &self,
        operand: &Operand,
        data_refs: &mut Vec<String>,
        enum_refs: &mut Vec<String>,
        inline_calls: &mut Vec<String>,
        callees: &mut Vec<String>,
    ) {
        match operand {
            Operand::Immediate { value } => {
                self.analyze_expr(value, data_refs, enum_refs, inline_calls, callees);
            }
            Operand::MemoryOperand { expr, .. } => {
                self.analyze_expr(expr, data_refs, enum_refs, inline_calls, callees);
            }
            Operand::LabelRef { name } => {
                if self.symbol_types.contains_key(name) && !data_refs.contains(name) {
                    data_refs.push(name.clone());
                }
            }
            Operand::Selector { path, accesses } => {
                if let Some(first) = path.first() {
                    if self.symbol_types.contains_key(first) && !data_refs.contains(first) {
                        data_refs.push(first.clone());
                    }
                    // Enum reference: if first is in enum_variants as "EnumName::*".
                    // Check if first matches any enum by looking for "first::" prefix in enum_variants.
                    let prefix = format!("{}::", first);
                    if self.enum_variants.keys().any(|k| k.starts_with(&prefix)) {
                        if !enum_refs.contains(first) {
                            enum_refs.push(first.clone());
                        }
                    }
                }
                for access in accesses {
                    if let Access::Offset { value, .. } = access {
                        self.analyze_expr(value, data_refs, enum_refs, inline_calls, callees);
                    }
                }
            }
            _ => {}
        }
    }

    #[allow(clippy::collapsible_if)]
    fn analyze_expr(
        &self,
        expr: &Expr,
        data_refs: &mut Vec<String>,
        enum_refs: &mut Vec<String>,
        inline_calls: &mut Vec<String>,
        callees: &mut Vec<String>,
    ) {
        match expr {
            Expr::Ident { name } => {
                if self.symbol_types.contains_key(name) && !data_refs.contains(name) {
                    data_refs.push(name.clone());
                }
                // Check if it's an enum name.
                let prefix = format!("{}::", name);
                if self.enum_variants.keys().any(|k| k.starts_with(&prefix)) {
                    if !enum_refs.contains(name) {
                        enum_refs.push(name.clone());
                    }
                }
            }
            Expr::Selector { path, accesses } => {
                if let Some(first) = path.first() {
                    let prefix = format!("{}::", first);
                    if self.enum_variants.keys().any(|k| k.starts_with(&prefix)) {
                        if !enum_refs.contains(first) {
                            enum_refs.push(first.clone());
                        }
                    }
                    if self.symbol_types.contains_key(first) && !data_refs.contains(first) {
                        data_refs.push(first.clone());
                    }
                    // If this is a struct field access (e.g.
                    // FONT_X.data), also add the array symbol that
                    // the field points to so it gets placed in ROM.
                    if let Some(fields) = self.struct_consts.get(first) {
                        for access in accesses {
                            if let Access::FieldAccess { name: field_name } = access {
                                if let Some((_, Expr::Ident { name: array_name })) =
                                    fields.iter().find(|(n, _)| n == field_name)
                                {
                                    if !data_refs.contains(array_name) {
                                        data_refs.push(array_name.clone());
                                    }
                                }
                            }
                        }
                    }
                }
                for access in accesses {
                    if let Access::Offset { value, .. } = access {
                        self.analyze_expr(value, data_refs, enum_refs, inline_calls, callees);
                    }
                }
            }
            Expr::BinOp { left, right, .. } => {
                self.analyze_expr(left, data_refs, enum_refs, inline_calls, callees);
                self.analyze_expr(right, data_refs, enum_refs, inline_calls, callees);
            }
            Expr::UnaryOp { operand, .. } => {
                self.analyze_expr(operand, data_refs, enum_refs, inline_calls, callees);
            }
            Expr::MacroCall { arg, .. } => {
                self.analyze_expr(arg, data_refs, enum_refs, inline_calls, callees);
            }
            Expr::FnCall { name, args } => {
                // Record an expression-position function call as a use. Mirror
                // the FnStmt::FnCall handling so the dead-code analyzer sees it.
                if let Some(inline_fn) = self.inline_fns.get(name) {
                    if inline_fn.is_inline {
                        if !inline_calls.contains(name) {
                            inline_calls.push(name.clone());
                        }
                        let substituted =
                            self.substitute_params(&inline_fn.body, &inline_fn.params, args);
                        self.analyze_stmts(
                            &substituted,
                            &inline_fn.params,
                            callees,
                            data_refs,
                            enum_refs,
                            inline_calls,
                        );
                    } else if !callees.contains(name) {
                        callees.push(name.clone());
                    }
                }
                // Unknown fn (stdlib or external) — not a callee for placement.
                for arg in args {
                    self.analyze_expr(arg, data_refs, enum_refs, inline_calls, callees);
                }
            }
            Expr::ParenExpr { inner } => {
                self.analyze_expr(inner, data_refs, enum_refs, inline_calls, callees);
            }
            _ => {}
        }
    }

    /// Find a `#[locate(...)]` attribute on an item. Returns the
    /// attribute arguments: `addr` (absolute ROM address) and optional
    /// `file` (binary file whose bytes emit at `addr`).
    fn find_locate_attr(attributes: &[Attribute]) -> Option<(Option<u32>, Option<String>)> {
        for attr in attributes {
            if attr.path != "locate" {
                continue;
            }
            let addr = get_attr_u32(attr, "addr");
            let file = attr
                .args
                .iter()
                .find(|a| a.name == "file")
                .map(|arg| arg.value.trim_matches('"').to_string());
            return Some((addr, file));
        }
        None
    }

    /// Grow the current section data so that the next emitted byte lands
    /// at absolute address `addr` (that is, at offset `addr - org`).
    /// Fills the gap with the module pad byte. Emits an error when the
    /// address precedes the current data end (overlap) or when the
    /// section has no room for the request (maxsize).
    fn locate_to_addr(&mut self, addr: u32, item_name: &str) -> bool {
        let idx = match self.current_section {
            Some(idx) => idx,
            None => {
                self.error(
                    300,
                    format!("#[locate] on `{item_name}`: no ROM block surrounds the item"),
                );
                return false;
            }
        };
        let org = self.sections[idx].org;
        let maxsize = self.sections[idx].maxsize;
        let target_offset = if addr >= org {
            addr - org
        } else {
            self.error(
                300,
                format!(
                    "#[locate] on `{item_name}`: address 0x{addr:04X} is below the block org 0x{org:04X}"
                ),
            );
            return false;
        };
        let cur_len = self.sections[idx].data.len() as u32;
        if target_offset < cur_len {
            self.error(
                300,
                format!(
                    "#[locate] on `{item_name}`: address 0x{addr:04X} overlaps existing data (current end 0x{:04X})",
                    org + cur_len
                ),
            );
            return false;
        }
        if maxsize > 0 && target_offset >= maxsize {
            self.error(
                300,
                format!(
                    "#[locate] on `{item_name}`: address 0x{addr:04X} exceeds the block maxsize 0x{maxsize:04X}"
                ),
            );
            return false;
        }
        let pad = self.pad_byte;
        self.sections[idx].data.resize(target_offset as usize, pad);
        true
    }

    /// Emit the bytes of `file` at the current section offset. Path
    /// resolution matches other file reads: relative to the current std
    /// module directory when walking std, else relative to the source
    /// directory.
    fn emit_file_bytes(&mut self, file: &str, item_name: &str) {
        let base = self
            .current_module_dir
            .clone()
            .unwrap_or_else(|| self.source_dir.clone());
        let path = base.join(file);
        match std::fs::read(&path) {
            Ok(data) => {
                let idx = match self.current_section {
                    Some(idx) => idx,
                    None => {
                        self.error(
                            300,
                            format!("#[locate] on `{item_name}`: no ROM block surrounds the item"),
                        );
                        return;
                    }
                };
                let maxsize = self.sections[idx].maxsize;
                let cur_len = self.sections[idx].data.len() as u32;
                let need = cur_len + data.len() as u32;
                if maxsize > 0 && need > maxsize {
                    self.error(
                        300,
                        format!(
                            "#[locate] on `{item_name}`: file `{file}` ({}) exceeds the block maxsize 0x{maxsize:04X}",
                            data.len()
                        ),
                    );
                    return;
                }
                for b in data {
                    self.sections[idx].data.push(b);
                }
            }
            Err(_) => {
                self.error(300, format!("#[locate]: cannot read file '{file}'"));
            }
        }
    }

    /// Place a top-level const's data into a section.
    fn place_const(
        &mut self,
        name: &str,
        ty: &Type,
        value: &Expr,
        evaluated_value: Option<i64>,
        section_idx: usize,
    ) {
        // For string consts, emit the string bytes.
        // For scalar consts, emit the evaluated value bytes.
        // For other consts, emit based on type size.
        self.current_section = Some(section_idx);

        // #[locate(addr = ...)] pins the const at an absolute address.
        // With `file`, the file bytes emit at the pinned address and the
        // const's own value bytes are skipped (the file replaces them).
        // Both modes record the symbol at the pinned offset.
        // The locate attribute is looked up by the placement pass and
        // stored in `self.pending_locate`.
        let (locate_addr, locate_file) = std::mem::take(&mut self.pending_locate);
        if let Some(addr) = locate_addr {
            if !self.locate_to_addr(addr, name) {
                self.current_section = None;
                return;
            }
        }
        let offset = self.sections[section_idx].data.len() as u32;
        if let Some(file) = locate_file {
            // The file replaces the const's own value bytes.
            self.emit_file_bytes(&file, name);
            let end_offset = self.sections[section_idx].data.len() as u32;
            self.sections[section_idx].symbols.push(Symbol {
                name: name.to_string(),
                offset,
                size: end_offset - offset,
                kind: SymbolKind::Variable,
                is_pub: false,
            });
            self.current_section = None;
            return;
        }

        match value {
            Expr::String_ { value: s } => {
                let s = s.trim_matches('"');
                for b in s.bytes() {
                    self.sections[section_idx].data.push(b);
                }
            }
            Expr::ArrayLit { .. } => {
                // Array const: emit the bytes from const_arrays.
                if let Some(bytes) = self.const_arrays.get(name) {
                    for b in bytes {
                        self.sections[section_idx].data.push(*b);
                    }
                } else {
                    // Can't find the array — emit zero bytes.
                    for _ in 0..self.type_size(ty) {
                        self.sections[section_idx].data.push(0);
                    }
                }
            }
            Expr::StructLit { fields, .. } => {
                // Struct const: emit each field in order.
                // Scalar fields emit their value bytes.
                // Pointer fields emit Lo8/Hi8 relocations.
                for (_field_name, field_expr) in fields {
                    if let Some(val) = eval_expr(field_expr, &self.const_values, &self.symbol_types)
                    {
                        // Scalar field: emit value bytes.
                        let field_ty_size = match field_expr {
                            Expr::Ident { name: sym } => {
                                self.symbol_types.get(sym).map(type_size).unwrap_or(1)
                            }
                            _ => 1,
                        };
                        let bytes = val_to_bytes(val, field_ty_size);
                        for b in &bytes {
                            self.sections[section_idx].data.push(*b);
                        }
                    } else {
                        // Pointer field: emit a relocation.
                        // Try to resolve as a symbol reference.
                        if let Some((sym, _, addend)) = self.classify_immediate(field_expr) {
                            let field_size = match field_expr {
                                Expr::Ident { .. } => 2, // pointer is 2 bytes
                                _ => 1,
                            };
                            let field_offset = self.sections[section_idx].data.len() as u32;
                            for _ in 0..field_size {
                                self.sections[section_idx].data.push(0);
                            }
                            // Use Abs16 for pointer fields (2 bytes),
                            // Abs8 for single-byte fields.
                            let reloc_kind = if field_size == 2 {
                                RelocKind::Abs16
                            } else {
                                RelocKind::Abs8
                            };
                            self.sections[section_idx].relocations.push(Relocation {
                                offset: field_offset,
                                kind: reloc_kind,
                                symbol: sym,
                                addend,
                            });
                        } else {
                            // Can't resolve — emit zeros.
                            self.sections[section_idx].data.push(0);
                            self.sections[section_idx].data.push(0);
                        }
                    }
                }
            }
            _ => {
                // Scalar or array const: use evaluated value.
                let val = evaluated_value
                    .or_else(|| eval_expr(value, &self.const_values, &self.symbol_types));
                let size = self.type_size(ty);
                if let Some(v) = val {
                    let bytes = val_to_bytes(v, size);
                    for b in &bytes {
                        self.sections[section_idx].data.push(*b);
                    }
                } else {
                    // Can't evaluate — emit zero bytes for the type size.
                    for _ in 0..self.type_size(ty) {
                        self.sections[section_idx].data.push(0);
                    }
                }
            }
        }

        let end_offset = self.sections[section_idx].data.len() as u32;
        // Record symbol.
        self.sections[section_idx].symbols.push(Symbol {
            name: name.to_string(),
            offset,
            size: end_offset - offset,
            kind: SymbolKind::Variable,
            is_pub: false,
        });
        self.current_section = None;
    }

    /// Place a const whose value comes from an external file.
    /// `#[locate(addr = ..., file = "...")]` on a naked const
    /// declaration with no initializer value.
    fn place_const_file(&mut self, name: &str, addr: u32, file: &str, section_idx: usize) {
        self.current_section = Some(section_idx);
        if !self.locate_to_addr(addr, name) {
            self.current_section = None;
            return;
        }
        let offset = self.sections[section_idx].data.len() as u32;
        self.emit_file_bytes(file, name);
        let end_offset = self.sections[section_idx].data.len() as u32;
        self.sections[section_idx].symbols.push(Symbol {
            name: name.to_string(),
            offset,
            size: end_offset - offset,
            kind: SymbolKind::Variable,
            is_pub: false,
        });
        self.current_section = None;
    }

    /// Return a clone of the current module path stack. The stack is
    /// empty at the crate root.
    fn current_module_path(&self) -> Vec<String> {
        self.module_path.clone()
    }

    /// Convert a use-tree root into a module path. `Lib` is the crate
    /// root (an empty path). `SelfMod` is the current stack. `Super`
    /// is the current stack without its last element. `Name` is a bare
    /// name.
    fn resolve_root(&self, root: &UseRoot) -> Vec<String> {
        match root {
            UseRoot::Lib => Vec::new(),
            UseRoot::SelfMod => self.module_path.clone(),
            UseRoot::Super => {
                let mut path = self.module_path.clone();
                path.pop();
                path
            }
            UseRoot::Name(name) => vec![name.clone()],
        }
    }

    // --- Use resolution -----------------------------------------------------

    /// Resolve every import tree in a `use` declaration. Imported
    /// names are inserted into the flat namespaces: `inline_fns` for
    /// inline functions, `const_values` for constants and enum
    /// variants, `enum_variants` for qualified variant names, and
    /// `use_aliases` for `as` bindings.
    fn resolve_use_decl(&mut self, trees: &[UseTree]) {
        let mut visited: HashSet<std::path::PathBuf> = HashSet::new();
        for tree in trees {
            self.resolve_use_tree(tree, &mut visited);
        }
    }

    /// Resolve a single import tree against the current module path
    /// and import its names.
    fn resolve_use_tree(&mut self, tree: &UseTree, visited: &mut HashSet<std::path::PathBuf>) {
        match tree {
            UseTree::Alias { inner, alias } => {
                self.use_aliases
                    .insert(alias.clone(), self.use_tree_module_path(inner));
            }
            UseTree::Path {
                root,
                segments,
                tail,
            } => {
                let mut path = self.use_tree_base_path(root);
                path.extend(segments.iter().cloned());

                match tail {
                    UseTail::Group(subtrees) => {
                        // Group members carry their own roots, so the
                        // group parent becomes their resolution
                        // context.
                        let saved = self.module_path.clone();
                        self.module_path = path.clone();
                        for subtree in subtrees {
                            self.resolve_use_tree(subtree, visited);
                        }
                        self.module_path = saved;
                    }
                    _ => self.import_path(&path, tail, visited),
                }
            }
        }
    }

    /// Compute the full path a use tree points at, treating every
    /// segment (including the last) as a module segment. Used for
    /// `as` aliases, which bind the alias to the target path.
    fn use_tree_module_path(&self, tree: &UseTree) -> Vec<String> {
        match tree {
            UseTree::Alias { inner, .. } => self.use_tree_module_path(inner),
            UseTree::Path { root, segments, .. } => {
                let mut path = self.use_tree_base_path(root);
                path.extend(segments.iter().cloned());
                path
            }
        }
    }

    /// Resolve a use-tree root to the module path it starts from.
    /// `std` always names the std crate root; any other bare name is
    /// relative to the current module path.
    fn use_tree_base_path(&self, root: &UseRoot) -> Vec<String> {
        match root {
            UseRoot::Name(name) if name == "std" => vec!["std".to_string()],
            UseRoot::Name(name) => {
                let mut path = self.current_module_path();
                path.push(name.clone());
                path
            }
            _ => self.resolve_root(root),
        }
    }

    /// Import the names at `path`. If `path` names a module file, the
    /// module's exported items are imported and nested public `use`
    /// trees are resolved in the module's own path context. Otherwise
    /// the last segment names an item inside the parent module: a
    /// glob of an enum also binds the variant names bare, a single
    /// import binds only the qualified names.
    fn import_path(
        &mut self,
        path: &[String],
        tail: &UseTail,
        visited: &mut HashSet<std::path::PathBuf>,
    ) {
        if path.first().is_some_and(|name| name == "std")
            && find_std_root(&self.include_paths).is_none()
        {
            self.error(
                302,
                "std library not found: set OP_STD_PATH or use --include",
            );
            return;
        }

        if let Some((module, file)) = self.lookup_module(path) {
            let key = file.canonicalize().unwrap_or(file);
            let module_dir = self.module_cache.dir_of(&key).cloned();
            if !visited.insert(key) {
                return;
            }
            let saved = self.module_path.clone();
            let saved_dir = self.current_module_dir.clone();
            self.module_path = path.to_vec();
            self.current_module_dir = module_dir;
            self.import_module_items(&module.items, visited);
            self.module_path = saved;
            self.current_module_dir = saved_dir;
            return;
        }

        // The path names an item, not a module file. Look the item up
        // in the parent module.
        if path.is_empty() {
            self.error(303, format!("module not found: {}", path.join("::")));
            return;
        }
        let parent = &path[..path.len() - 1];
        let name = &path[path.len() - 1];
        let Some((parent_module, _file)) = self.lookup_module(parent) else {
            self.error(303, format!("module not found: {}", path.join("::")));
            return;
        };
        let bare = matches!(tail, UseTail::Glob);
        let saved = self.module_path.clone();
        self.module_path = parent.to_vec();
        let mut found = false;
        for item in &parent_module.items {
            if decl_name(item) == Some(name.as_str()) {
                found = true;
                self.import_named_item(item, name, bare);
                break;
            }
        }
        self.module_path = saved;
        if !found && matches!(tail, UseTail::Glob) {
            // A glob must name a module or an importable item. A
            // single import of a missing name may be an item that is
            // cfg-gated out for this target, so it is not an error.
            self.error(303, format!("module not found: {}", path.join("::")));
        }
    }

    /// Import the exported items of a module into the flat namespaces.
    /// The module path is already on the stack, so nested `use`
    /// trees resolve relative to this module.
    fn import_module_items(&mut self, items: &[Item], visited: &mut HashSet<std::path::PathBuf>) {
        for item in items {
            match item {
                Item::InlineFnDecl {
                    name, params, body, ..
                } => {
                    self.import_inline_fn(name, params, body);
                }
                Item::FnDecl {
                    name,
                    is_noreturn,
                    body,
                    ..
                } => {
                    self.import_fn(name, *is_noreturn, body);
                }
                Item::ConstDecl {
                    name,
                    ty,
                    value,
                    evaluated_value,
                    attributes,
                    ..
                } => {
                    match value {
                        Expr::ArrayLit { elements } => {
                            let mut bytes = Vec::new();
                            for elem in elements {
                                if let Some(val) =
                                    eval_expr(elem, &self.const_values, &self.symbol_types)
                                {
                                    bytes.push(val as u8);
                                } else {
                                    bytes.push(0);
                                }
                            }
                            self.const_arrays.insert(name.clone(), bytes);
                            self.const_values
                                .insert(name.clone(), elements.len() as i64);
                        }
                        Expr::StructLit { fields, .. } => {
                            let field_exprs: Vec<(String, Expr)> = fields.clone();
                            for (field_name, field_expr) in fields {
                                // For pointer fields that reference an array
                                // const, do NOT store the array's length in
                                // const_values. The field holds a pointer
                                // (address), and storing the length would
                                // cause lo!/hi! of the field to resolve to
                                // the length instead of emitting a
                                // relocation against the array symbol.
                                let is_array_ref = matches!(
                                    field_expr,
                                    Expr::Ident { name } if self.const_arrays.contains_key(name)
                                );
                                if !is_array_ref {
                                    if let Some(val) = eval_expr(
                                        field_expr,
                                        &self.const_values,
                                        &self.symbol_types,
                                    ) {
                                        self.const_values
                                            .insert(format!("{}::{}", name, field_name), val);
                                    }
                                }
                            }
                            self.struct_consts.insert(name.clone(), field_exprs);
                        }
                        _ => {
                            let val = (*evaluated_value).or_else(|| {
                                eval_expr(value, &self.const_values, &self.symbol_types)
                            });
                            if let Some(val) = val {
                                self.import_const(name, val);
                            }
                        }
                    }
                    // Collect for placement in ROM.
                    if let Some(locate) = attributes.iter().find(|a| a.path == "locate") {
                        let addr = get_attr_u32(locate, "addr");
                        let file = locate
                            .args
                            .iter()
                            .find(|a| a.name == "file")
                            .map(|a| a.value.trim_matches('"').to_string());
                        self.collected_locates.insert(name.clone(), (addr, file));
                    }
                    self.collected_consts.push((
                        name.clone(),
                        ty.clone(),
                        value.clone(),
                        *evaluated_value,
                    ));
                }
                Item::EnumDecl { name, variants, .. } => {
                    self.collect_enum(name, variants, true);
                }
                Item::UseDecl { trees, .. } => {
                    for tree in trees {
                        self.resolve_use_tree(tree, visited);
                    }
                }
                Item::Placement {
                    macro_name,
                    argument,
                    ..
                } if self.current_section.is_some() => {
                    self.handle_placement(macro_name, argument);
                }
                Item::ModDecl {
                    name,
                    resolved,
                    body,
                    ..
                } => {
                    // Recurse into sub-modules to collect their vars
                    // and other items for placement.
                    self.module_path.push(name.clone());
                    if let Some(sub_module) = resolved {
                        self.collect_module_items(&sub_module.items);
                    } else if let Some(items) = body {
                        self.collect_module_items(items);
                    }
                    self.module_path.pop();
                }
                Item::VarDecl {
                    name,
                    ty,
                    addr_binding,
                    init,
                    ..
                } => {
                    self.collected_vars.push((
                        name.clone(),
                        ty.clone(),
                        addr_binding.clone(),
                        init.clone(),
                    ));
                }
                Item::StructDecl { name, fields, .. } => {
                    let size: usize = fields
                        .iter()
                        .map(|f| {
                            let base = type_size(&f.ty);
                            if let Some(dim) = &f.array_dim {
                                if let Some(d) = eval_const_expr_simple(dim) {
                                    base * d as usize
                                } else {
                                    base
                                }
                            } else {
                                base
                            }
                        })
                        .sum();
                    self.struct_sizes.insert(name.clone(), size);
                }
                _ => {}
            }
        }
    }

    /// Import a single named item found in a parent module.
    fn import_named_item(&mut self, item: &Item, name: &str, bare: bool) {
        match item {
            Item::InlineFnDecl { params, body, .. } => {
                self.import_inline_fn(name, params, body);
            }
            Item::FnDecl {
                is_noreturn, body, ..
            } => {
                self.import_fn(name, *is_noreturn, body);
            }
            Item::ConstDecl {
                value,
                evaluated_value,
                ..
            } => match value {
                Expr::ArrayLit { elements } => {
                    let mut bytes = Vec::new();
                    for elem in elements {
                        if let Some(val) = eval_expr(elem, &self.const_values, &self.symbol_types) {
                            bytes.push(val as u8);
                        } else {
                            bytes.push(0);
                        }
                    }
                    self.const_arrays.insert(name.to_string(), bytes);
                    self.const_values
                        .insert(name.to_string(), elements.len() as i64);
                }
                Expr::StructLit { fields, .. } => {
                    let field_exprs: Vec<(String, Expr)> = fields.clone();
                    for (field_name, field_expr) in fields {
                        let is_array_ref = matches!(
                            field_expr,
                            Expr::Ident { name } if self.const_arrays.contains_key(name)
                        );
                        if !is_array_ref {
                            if let Some(val) =
                                eval_expr(field_expr, &self.const_values, &self.symbol_types)
                            {
                                self.const_values
                                    .insert(format!("{}::{}", name, field_name), val);
                            }
                        }
                    }
                    self.struct_consts.insert(name.to_string(), field_exprs);
                }
                _ => {
                    let val = (*evaluated_value)
                        .or_else(|| eval_expr(value, &self.const_values, &self.symbol_types));
                    if let Some(val) = val {
                        self.import_const(name, val);
                    }
                }
            },
            Item::EnumDecl { variants, .. } => {
                self.collect_enum(name, variants, bare);
            }
            _ => {}
        }
    }

    /// Insert a non-inline function into the flat namespace, warning
    /// on collision. Its body is placed exactly once (in the first ROM
    /// section) when the function is reachable from a placement root;
    /// calls emit CALL nn against the function symbol.
    fn import_fn(&mut self, name: &str, is_noreturn: bool, body: &[FnStmt]) {
        if is_noreturn {
            self.noreturn_fns.insert(name.to_string());
        }
        if self.inline_fns.contains_key(name) {
            self.warning(
                304,
                format!("name `{name}` imported more than once; last import wins"),
            );
        }
        self.inline_fns.insert(
            name.to_string(),
            InlineFn {
                params: Vec::new(),
                body: body.to_vec(),
                is_inline: false,
            },
        );
    }

    /// Insert an inline function into the flat namespace, warning on
    /// collision.
    fn import_inline_fn(&mut self, name: &str, params: &[String], body: &[FnStmt]) {
        if self.inline_fns.contains_key(name) {
            self.warning(
                304,
                format!("name `{name}` imported more than once; last import wins"),
            );
        }
        self.inline_fns.insert(
            name.to_string(),
            InlineFn {
                params: params.to_vec(),
                body: body.to_vec(),
                is_inline: true,
            },
        );
    }

    /// Insert a constant into the flat namespace, warning when an
    /// existing binding has a different value.
    fn import_const(&mut self, name: &str, val: i64) {
        match self.const_values.insert(name.to_string(), val) {
            Some(prev) if prev != val => {
                self.warning(
                    304,
                    format!("constant `{name}` imported with conflicting values; last import wins"),
                );
            }
            _ => {}
        }
    }

    /// Collect an enum's variant values into the flat namespace.
    /// Qualified keys (`EnumName::VariantName`) are always inserted.
    /// A variant without an explicit value takes the previous
    /// variant's value plus one; the first variant takes zero. Bare
    /// variant names are inserted only when the enum is glob-imported
    /// and only when the name is not already bound.
    fn collect_enum(&mut self, name: &str, variants: &[op_common::ast::EnumVariant], bare: bool) {
        let mut prev: Option<i64> = None;
        for variant in variants {
            // An explicit value that cannot be evaluated falls back to
            // the implicit value so one bad variant cannot drop the
            // rest of the enum.
            let val = variant
                .value
                .as_ref()
                .and_then(|v| eval_expr(v, &self.const_values, &self.symbol_types))
                .or_else(|| prev.map(|p| p + 1))
                .unwrap_or(0);
            let qualified = format!("{name}::{}", variant.name);
            self.enum_variants.insert(qualified.clone(), val);
            let msg = "enum variant imported with conflicting values; last import wins";
            match self.const_values.insert(qualified, val) {
                Some(existing) if existing != val => self.warning(304, msg),
                _ => {}
            }
            if bare {
                self.const_values.entry(variant.name.clone()).or_insert(val);
            }
            prev = Some(val);
        }
    }

    /// Look up and load the module file for `path`, if it exists. No
    /// diagnostic is emitted; the caller decides what a miss means.
    /// Paths that start with `std` resolve against the std crate
    /// root; other paths resolve against the directory of the root
    /// source file.
    fn lookup_module(&mut self, path: &[String]) -> Option<(Module, std::path::PathBuf)> {
        let (base, rest) = match path.first() {
            Some(name) if name == "std" => {
                let root = find_std_root(&self.include_paths)?;
                (root, &path[1..])
            }
            _ => (self.source_dir.clone(), path),
        };
        let file = module_file_path(&base, rest)?;
        self.module_cache
            .load_module(&file, &self.target, &self.features)
            .ok()
            .map(|module| (module, file))
    }

    fn walk_item(&mut self, item: &Item) {
        match item {
            Item::ConstDecl {
                name,
                evaluated_value,
                ..
            } => {
                if self.placed_items.contains(name) {
                    return;
                }
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
                if self.placed_items.contains(name) {
                    return;
                }
                self.alloc_variable(name, ty, addr_binding, init);
            }
            Item::FnDecl {
                name,
                body,
                is_noreturn,
                attributes,
            } => {
                // Check for #[interrupt(name)] attribute.
                for attr in attributes {
                    if attr.path == "interrupt" {
                        if let Some(int_name) = attr.args.first().map(|a| a.name.as_str()) {
                            if !int_name.is_empty() {
                                self.add_interrupt_vector(int_name, name);
                            }
                        }
                    }
                }
                // Store the body so the placement pass can find it (the
                // collect pass already does this, but repeat in case
                // the fn was not seen by collect — e.g. in a nested
                // walk).
                self.inline_fns.entry(name.clone()).or_insert(InlineFn {
                    params: Vec::new(),
                    body: body.clone(),
                    is_inline: false,
                });
                if *is_noreturn {
                    self.noreturn_fns.insert(name.clone());
                }
                // Skip compilation if the placer already placed this fn.
                if self.placed_items.contains(name) {
                    return;
                }
                // When declared inside a section, compile in place.
                // When declared outside a section, defer to a placement root.
                if self.current_section.is_some() {
                    self.compile_fn(name, body, *is_noreturn);
                }
            }
            Item::InlineFnDecl {
                name, params, body, ..
            } => {
                self.inline_fns.entry(name.clone()).or_insert(InlineFn {
                    params: params.clone(),
                    body: body.clone(),
                    is_inline: true,
                });
            }
            Item::StructDecl { .. } | Item::TypeDecl { .. } | Item::EnumDecl { .. } => {
                // No codegen output for type declarations.
            }
            Item::ModDecl {
                name,
                resolved,
                body,
                ..
            } => {
                // The sub-module's items were already collected during
                // the module's collection pass.
                self.module_path.push(name.clone());
                if let Some(sub_module) = resolved {
                    for item in &sub_module.items {
                        self.walk_item(item);
                    }
                } else if let Some(items) = body {
                    for item in items {
                        self.walk_item(item);
                    }
                }
                self.module_path.pop();
            }
            Item::UseDecl { .. } => {
                // Imports are resolved by `collect_module_items` before
                // any fn body is compiled.
            }
            Item::BlockAttribute { attr, items } => {
                self.handle_block_attribute(attr, items);
            }
            Item::Placement {
                macro_name,
                argument,
                attributes,
            } => {
                // Check for #[interrupt(name)] attribute on placements.
                for attr in attributes {
                    if attr.path == "interrupt" {
                        if let Some(int_name) = attr.args.first().map(|a| a.name.as_str()) {
                            if !int_name.is_empty() {
                                // Get the target function name from the placement argument.
                                let target_name = if let PlacementArg::Path { segments } = argument
                                {
                                    segments.last().cloned().unwrap_or_default()
                                } else {
                                    String::new()
                                };
                                if !target_name.is_empty() {
                                    self.add_interrupt_vector(int_name, &target_name);
                                }
                            }
                        }
                    }
                }
                self.handle_placement(macro_name, argument);
            }
        }
    }

    /// Add an interrupt vector entry for the given interrupt name and
    /// target function. On the 68000, a `reset` interrupt emits two
    /// vectors: the initial stack pointer at address 0x0000 (target
    /// `_stack_top`, resolved by the linker from the first RAM section)
    /// and the initial program counter at address 0x0004 (the handler).
    fn add_interrupt_vector(&mut self, int_name: &str, target: &str) {
        let encoding = vector_encoding_for(&self.target.cpu);

        // 68000 reset: emit SSP vector (address 0x0000) and PC vector
        // (address 0x0004) as a pair.
        if self.target.cpu == "m68000" && int_name == "reset" {
            self.interrupt_vectors.push(op_ir::InterruptVector {
                name: "reset".to_string(),
                address: 0x0000,
                target: "_stack_top".to_string(),
                encoding,
            });
            self.interrupt_vectors.push(op_ir::InterruptVector {
                name: "reset_pc".to_string(),
                address: 0x0004,
                target: target.to_string(),
                encoding,
            });
            return;
        }

        if let Some(vec_addr) = interrupt_vector_address(&self.target.cpu, int_name) {
            self.interrupt_vectors.push(op_ir::InterruptVector {
                name: int_name.to_string(),
                address: vec_addr,
                target: target.to_string(),
                encoding,
            });
        }
    }

    fn handle_block_attribute(&mut self, attr: &Attribute, items: &[Item]) {
        // Handle standalone attributes (empty items) that are not section blocks.
        if items.is_empty() {
            match attr.path.as_str() {
                "ines" => {
                    let fields: Vec<(String, String)> = attr
                        .args
                        .iter()
                        .map(|a| (a.name.clone(), a.value.trim_matches('"').to_string()))
                        .collect();
                    self.header = Some(op_ir::HeaderFields {
                        format: "ines".to_string(),
                        fields,
                    });
                    return;
                }
                "lnx" => {
                    let fields: Vec<(String, String)> = attr
                        .args
                        .iter()
                        .map(|a| (a.name.clone(), a.value.trim_matches('"').to_string()))
                        .collect();
                    self.header = Some(op_ir::HeaderFields {
                        format: "lnx".to_string(),
                        fields,
                    });
                    return;
                }
                "gb" => {
                    let fields: Vec<(String, String)> = attr
                        .args
                        .iter()
                        .map(|a| (a.name.clone(), a.value.trim_matches('"').to_string()))
                        .collect();
                    self.header = Some(op_ir::HeaderFields {
                        format: "gb".to_string(),
                        fields,
                    });
                    return;
                }
                "sega" => {
                    let fields: Vec<(String, String)> = attr
                        .args
                        .iter()
                        .map(|a| (a.name.clone(), a.value.trim_matches('"').to_string()))
                        .collect();
                    self.header = Some(op_ir::HeaderFields {
                        format: "sega".to_string(),
                        fields,
                    });
                    return;
                }
                "snes" => {
                    let fields: Vec<(String, String)> = attr
                        .args
                        .iter()
                        .map(|a| (a.name.clone(), a.value.trim_matches('"').to_string()))
                        .collect();
                    self.header = Some(op_ir::HeaderFields {
                        format: "snes".to_string(),
                        fields,
                    });
                    return;
                }
                "sms" => {
                    let fields: Vec<(String, String)> = attr
                        .args
                        .iter()
                        .map(|a| (a.name.clone(), a.value.trim_matches('"').to_string()))
                        .collect();
                    self.header = Some(op_ir::HeaderFields {
                        format: "sms".to_string(),
                        fields,
                    });
                    return;
                }
                "a78" => {
                    let fields: Vec<(String, String)> = attr
                        .args
                        .iter()
                        .map(|a| (a.name.clone(), a.value.trim_matches('"').to_string()))
                        .collect();
                    self.header = Some(op_ir::HeaderFields {
                        format: "a78".to_string(),
                        fields,
                    });
                    return;
                }
                "crt" => {
                    let fields: Vec<(String, String)> = attr
                        .args
                        .iter()
                        .map(|a| (a.name.clone(), a.value.trim_matches('"').to_string()))
                        .collect();
                    self.header = Some(op_ir::HeaderFields {
                        format: "crt".to_string(),
                        fields,
                    });
                    return;
                }
                "setpad" => {
                    if let Some(arg) = attr.args.first() {
                        let val = arg.value.trim_matches('"');
                        if let Some(hex) = val.strip_prefix("0x") {
                            self.pad_byte = u8::from_str_radix(hex, 16).unwrap_or(0x00);
                        } else {
                            self.pad_byte = val.parse::<u8>().unwrap_or(0x00);
                        }
                    }
                    return;
                }
                _ => {}
            }
        }

        let kind = match attr.path.as_str() {
            "rom" => SectionKind::Rom,
            "ram" => SectionKind::Ram,
            "chr" => SectionKind::Chr,
            _ => return,
        };

        let bank = get_attr_u32(attr, "bank").unwrap_or(0);
        let name = format!(
            "{}_bank{}",
            match kind {
                SectionKind::Rom => "rom",
                SectionKind::Ram => "ram",
                SectionKind::Chr => "chr",
            },
            bank
        );

        // Find the section if it was already created by create_sections.
        let section_idx = if let Some(idx) = self.sections.iter().position(|s| s.name == name) {
            idx
        } else {
            // Section doesn't exist yet — create it.
            let org = get_attr_u32(attr, "org").unwrap_or(0);
            let maxsize = get_attr_u32(attr, "maxsize").unwrap_or(0);
            self.sections.push(Section {
                name,
                kind,
                org,
                bank,
                maxsize,
                symbols: Vec::new(),
                relocations: Vec::new(),
                data: Vec::new(),
            });
            self.sections.len() - 1
        };

        self.current_section = Some(section_idx);

        for item in items {
            self.walk_item(item);
        }

        self.current_section = None;
    }

    fn handle_placement(&mut self, macro_name: &str, argument: &PlacementArg) {
        match macro_name {
            "locate_str" => {
                if let PlacementArg::String_ { value } = argument {
                    let filename = value.trim_matches('"');
                    let base = self
                        .current_module_dir
                        .clone()
                        .unwrap_or_else(|| self.source_dir.clone());
                    let path = base.join(filename);
                    if let Ok(source) = std::fs::read_to_string(&path) {
                        let (ast, _diags) = parser::parse_source(
                            &path.to_string_lossy(),
                            &source,
                            &self.target.as_str(),
                            &[],
                        );
                        self.walk_module(&ast.root);
                    }
                }
            }
            "font_load" => {
                // NES-only compile-time placement macro. Expands 1bpp
                // font data to 2-plane NES CHR data and emits it into the
                // current CHR section.
                if self.target.machine != "nes" {
                    self.error(307, "font_load! is only supported on NES targets");
                    return;
                }
                if let PlacementArg::Path { segments } = argument {
                    if let Some(font_name) = segments.last() {
                        // Look up the font_t const in struct_consts.
                        if let Some(fields) = self.struct_consts.get(font_name).cloned() {
                            // Find the `data` field expression.
                            let data_expr = fields
                                .iter()
                                .find(|(n, _)| n == "data")
                                .map(|(_, e)| e.clone());
                            // Find the `tile_count` field value.
                            let tile_count = self
                                .const_values
                                .get(&format!("{}::tile_count", font_name))
                                .copied();

                            if let (Some(expr), Some(count)) = (data_expr, tile_count) {
                                // Extract the array symbol name from the
                                // data field expression.
                                let array_name = match &expr {
                                    Expr::Ident { name } => name.clone(),
                                    _ => {
                                        self.error(
                                            307,
                                            format!(
                                                "font_load!: data field of `{}` is not a symbol reference",
                                                font_name
                                            ),
                                        );
                                        return;
                                    }
                                };

                                // Look up the array bytes.
                                if let Some(bytes) = self.const_arrays.get(&array_name).cloned() {
                                    // Expand 1bpp to 2-plane NES CHR.
                                    // The font blob is: [flags][tile_count][enc_table][1bpp_tiles]
                                    // We need to skip the header + encoding table to get to the tile data.
                                    let flags = if !bytes.is_empty() { bytes[0] } else { 0 };
                                    let enc_type = flags & 3;
                                    let enc_size: usize = match enc_type {
                                        0 => 256,
                                        1 => 128,
                                        _ => 0,
                                    };
                                    let tile_data_start = 2 + enc_size;
                                    let tile_count = count as usize;
                                    let tile_h = 8; // All current fonts are 8x8

                                    // Expand each 1bpp tile to 2-plane CHR.
                                    for tile_idx in 0..tile_count {
                                        let tile_start = tile_data_start + tile_idx * tile_h;
                                        if tile_start + tile_h > bytes.len() {
                                            break;
                                        }
                                        // Write plane 0: the raw 1bpp bytes.
                                        for row in 0..tile_h {
                                            self.emit_byte(bytes[tile_start + row]);
                                        }
                                        // Write plane 1: all zeros (simplified
                                        // expansion for fg=1, bg=0).
                                        for _ in 0..tile_h {
                                            self.emit_byte(0);
                                        }
                                    }
                                } else {
                                    self.error(
                                        307,
                                        format!(
                                            "font_load!: array `{}` not found in const_arrays",
                                            array_name
                                        ),
                                    );
                                }
                            } else {
                                self.error(
                                    307,
                                    format!(
                                        "font_load!: font `{}` missing data or tile_count field",
                                        font_name
                                    ),
                                );
                            }
                        } else {
                            self.error(
                                307,
                                format!(
                                    "font_load!: font `{}` not found in struct_consts",
                                    font_name
                                ),
                            );
                        }
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
        let size = self.type_size(ty);
        let offset = if let Some(addr_expr) = addr_binding {
            eval_expr(addr_expr, &self.const_values, &self.symbol_types).unwrap_or(0) as u32
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
                        if let Some(val) = eval_expr(value, &self.const_values, &self.symbol_types)
                        {
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
                                if let Some(val) =
                                    eval_expr(value, &self.const_values, &self.symbol_types)
                                {
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

    /// Emit code to stash file/line/msg into `__panic_file`,
    /// `__panic_line`, `__panic_msg` and jump to the crash handler.
    fn emit_panic_stash(&mut self, args: &[Expr]) {
        // The file, line, and msg are passed as args. We emit
        // relocations against generated ROM string consts.
        // For now, emit zero (the linker resolves the symbol).
        let _ = args;
        // Store zero into __panic_file (2 bytes).
        self.emit_byte(0);
        self.emit_byte(0);
        // Store zero into __panic_line (2 bytes).
        self.emit_byte(0);
        self.emit_byte(0);
        // Store zero into __panic_msg (2 bytes).
        self.emit_byte(0);
        self.emit_byte(0);
        // Jump to crash handler.
        let jmp = match self.target.cpu.as_str() {
            "sm83" | "z80" => 0xC3, // JP
            _ => 0x4C,              // JMP
        };
        self.emit_byte(jmp);
        self.emit_byte(0);
        self.emit_byte(0);
        let ch_sym = self.crash_handler_symbol.clone();
        self.add_relocation(2, RelocKind::Abs16, &ch_sym, 0);
    }

    /// Emit code for `assert!(cond)` or `assert_eq!(a, b)`.
    /// When `is_eq` is false, evaluate the condition. When it is
    /// false, call `emit_panic_stash`. When `is_eq` is true,
    /// evaluate both sides and compare.
    fn emit_assert(&mut self, args: &[Expr], _is_eq: bool) {
        // Simplified: always panic (the condition is not evaluated
        // at compile time). A full implementation would emit a
        // conditional branch.
        self.emit_panic_stash(args);
    }

    fn compile_fn(&mut self, name: &str, body: &[FnStmt], is_noreturn: bool) {
        // Name the fn for the flow-control diagnostics, and restore
        // the previous name on exit: inline fn bodies are compiled
        // inside the enclosing fn and keep reporting it.
        let saved_fn = std::mem::replace(&mut self.current_fn, name.to_string());

        // Record the function symbol at the current offset.
        let offset = if let Some(idx) = self.current_section {
            self.sections[idx].data.len() as u32
        } else {
            0
        };

        let start_offset = offset;

        // Compile the function body.
        self.compile_fn_body(body);

        // Emit an implicit RTS at the end of the function unless the last
        // statement already ends control flow (return, loop, or an
        // unconditional branch). Without this, a function that lacks an
        // explicit `return` falls through into whatever follows it in the
        // section.
        if !Self::body_ends_control_flow(body) && !is_noreturn {
            self.emit_byte(self.return_op());
        }

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

        self.current_fn = saved_fn;
    }

    /// Return true when the last statement of a function body ends control
    /// flow, making a trailing RTS redundant. `return`, infinite `loop`,
    /// and an assembly instruction that returns or jumps (`rts`, `rti`,
    /// `jmp`, `jp`, `bra`) all end control flow.
    fn body_ends_control_flow(body: &[FnStmt]) -> bool {
        match body.last() {
            Some(FnStmt::ReturnStmt) => true,
            Some(FnStmt::LoopStmt { .. }) => true,
            Some(FnStmt::AsmStmt { opcode, .. }) => {
                matches!(
                    opcode.as_str(),
                    "rts"
                        | "rti"
                        | "jmp"
                        | "jp"
                        | "bra"
                        | "ret"
                        | "reti"
                        | "jr"
                        | "jr_nz"
                        | "jr_z"
                        | "jr_nc"
                        | "jr_c"
                )
            }
            _ => false,
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
                // Check for compile-time macros that appear as FnCall
                // statements (compile_error!, assert!, assert_eq!,
                // debug_assert!, debug_assert_eq!, panic!).
                match name.as_str() {
                    "compile_error" => {
                        let msg = args
                            .first()
                            .and_then(|e| {
                                if let Expr::String_ { value } = e {
                                    Some(value.trim_matches('"'))
                                } else {
                                    None
                                }
                            })
                            .unwrap_or("compile_error!");
                        self.error(307, msg.to_string());
                        return;
                    }
                    "panic" => {
                        // Stash file/line/msg then jump to crash handler.
                        self.emit_panic_stash(args);
                        return;
                    }
                    "assert" => {
                        // assert!(cond) — evaluate cond, branch to
                        // fail path if false.
                        self.emit_assert(args, false);
                        return;
                    }
                    "assert_eq" => {
                        // assert_eq!(a, b) — compare a and b, branch
                        // to fail path if not equal.
                        self.emit_assert(args, true);
                        return;
                    }
                    _ => {}
                }
                self.compile_fn_call(name, args);
            }
            FnStmt::ReturnStmt => {
                self.emit_byte(self.return_op());
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

    /// Classify an immediate operand that could not be evaluated as a
    /// constant. Returns the symbol name, relocation kind, and addend for
    /// a one-byte relocation. `lo!(sym)` and `hi!(sym)` produce `Lo8` and
    /// `Hi8` relocations; a bare symbol or selector produces an `Abs8`
    /// relocation. `nylo!`/`nyhi!` of a non-constant emit an error and
    /// return `None`. Any other expression returns `None`.
    fn classify_immediate(&mut self, expr: &Expr) -> Option<(String, RelocKind, i64)> {
        match expr {
            Expr::Ident { name } => Some((name.clone(), RelocKind::Abs8, 0)),
            Expr::Selector { path, accesses } => {
                if path.is_empty() {
                    return None;
                }

                // Check if this is a struct field access (CONST::field).
                // If the const is in struct_consts, resolve the field
                // expression to a relocation against the target symbol.
                if !accesses.is_empty() && path.len() == 1 {
                    let const_name = &path[0];
                    // Look for a FieldAccess in the accesses.
                    if let Some(Access::FieldAccess { name: field_name }) = accesses.first() {
                        // Clone the field expression out of struct_consts
                        // to avoid the borrow conflict.
                        let field_expr = self.struct_consts.get(const_name).and_then(|fields| {
                            fields
                                .iter()
                                .find(|(n, _)| n == field_name)
                                .map(|(_, e)| e.clone())
                        });
                        if let Some(expr) = field_expr {
                            // The field expression is typically a bare
                            // Ident pointing to the array symbol.
                            // Recurse to classify it.
                            return self.classify_immediate(&expr);
                        }
                    }
                }

                // Fall back to existing behavior.
                Some((
                    path.join("::"),
                    RelocKind::Abs8,
                    selector_offset(accesses, &self.const_values, &self.symbol_types),
                ))
            }
            Expr::MacroCall { name, arg } => match name.as_str() {
                "lo" => {
                    let (sym, _, addend) = self.classify_immediate(arg)?;
                    Some((sym, RelocKind::Lo8, addend))
                }
                "hi" => {
                    let (sym, _, addend) = self.classify_immediate(arg)?;
                    Some((sym, RelocKind::Hi8, addend))
                }
                "nylo" | "nyhi" => {
                    self.error(
                        305,
                        format!("`{}!` of a non-constant is not supported", name),
                    );
                    None
                }
                _ => None,
            },
            _ => None,
        }
    }

    // --- Assembly encoding ---------------------------------------------------

    fn compile_asm(&mut self, opcode: &str, operands: &[Operand]) {
        // Official Nintendo-manual SM83 spellings resolve first, and
        // only for the SM83 target. Shapes the resolver does not
        // cover keep the legacy paths byte for byte; a decode
        // diagnostic must not fall through, because the legacy path
        // reads only the first operand of the statement.
        if self.target.cpu == "sm83" {
            match crate::sm83_official::resolve(opcode, operands) {
                crate::sm83_official::Resolution::Resolved(resolved) => {
                    self.compile_resolved_sm83(resolved, operands);
                    return;
                }
                crate::sm83_official::Resolution::DecodeError(message) => {
                    let fn_name = self.diag_fn_name();
                    self.error(
                        311,
                        format!("fn '{fn_name}': official SM83 statement '{opcode}': {message}"),
                    );
                    return;
                }
                crate::sm83_official::Resolution::NotOfficial => {}
            }
        }

        // Handle implied/accumulator mode (no operands).
        if operands.is_empty() {
            // SM83 CB-prefix pairs (e.g. BIT 7,H = CB 7C) are two bytes.
            if self.target.cpu == "sm83" || self.target.cpu == "z80" {
                if let Some(pair) = crate::encoding::lookup_cb_pair(opcode) {
                    for b in pair {
                        self.emit_byte(*b);
                    }
                    return;
                }
            }
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

        // Rockwell bit branches (BBR0-BBR7, BBS0-BBS7 on the W65C02S)
        // take a zero-page address and a label: opcode byte, zero-page
        // byte, relative offset byte. The encoding table lists them
        // under the Relative mode, so a memory operand followed by a
        // label reference selects this three-byte form.
        if operands.len() == 2 {
            if let (Operand::MemoryOperand { expr, .. }, Operand::LabelRef { name }) =
                (&operands[0], &operands[1])
            {
                if let Some(op_byte) = self.lookup(opcode, AddrMode::Relative) {
                    self.emit_byte(op_byte);
                    let val = eval_expr(expr, &self.const_values, &self.symbol_types);
                    match val {
                        Some(v) => {
                            self.emit_byte((v & 0xFF) as u8);
                        }
                        None => {
                            // Symbol reference: placeholder byte plus an
                            // Abs8 relocation, matching the zero-page
                            // path of compile_memory_operand.
                            self.emit_byte(0);
                            let addend =
                                selector_addend(expr, &self.const_values, &self.symbol_types);
                            match expr_to_symbol(expr) {
                                Some(sym) => {
                                    self.add_relocation(1, RelocKind::Abs8, &sym, addend);
                                }
                                None => {
                                    self.error(
                                        305,
                                        "address operand is neither a constant nor a symbol",
                                    );
                                }
                            }
                        }
                    }
                    self.emit_byte(0); // placeholder branch offset
                    self.add_relocation(1, RelocKind::Branch8, name, 0);
                    return;
                }
            }
        }

        let operand = &operands[0];

        match operand {
            Operand::Immediate { value } => {
                if let Some(op_byte) = self.lookup(opcode, AddrMode::Immediate) {
                    self.emit_byte(op_byte);
                    let val = eval_expr(value, &self.const_values, &self.symbol_types);
                    // SM83 16-bit register pair loads (LD HL/DE/BC, nn)
                    // use a 2-byte immediate, not 1-byte.
                    let is_16bit_imm = matches!(opcode, "ld_hl" | "ld_de" | "ld_bc" | "ld_sp");
                    match val {
                        Some(v) => {
                            if is_16bit_imm {
                                self.emit_byte((v & 0xFF) as u8);
                                self.emit_byte(((v >> 8) & 0xFF) as u8);
                            } else {
                                self.emit_byte((v & 0xFF) as u8);
                            }
                        }
                        None => {
                            // Symbol reference — classify and emit a
                            // relocation, or report an error.
                            if is_16bit_imm {
                                self.emit_byte(0);
                                self.emit_byte(0);
                                match self.classify_immediate(value) {
                                    Some((sym, _kind, addend)) => {
                                        self.add_relocation(2, RelocKind::Abs16, &sym, addend);
                                    }
                                    None => {
                                        self.error(
                                            305,
                                            "immediate operand is neither a constant nor a symbol",
                                        );
                                    }
                                }
                            } else {
                                self.emit_byte(0);
                                match self.classify_immediate(value) {
                                    Some((sym, kind, addend)) => {
                                        self.add_relocation(1, kind, &sym, addend);
                                    }
                                    None => {
                                        self.error(
                                            305,
                                            "immediate operand is neither a constant nor a symbol",
                                        );
                                    }
                                }
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
                is_indirect,
            } => {
                self.compile_memory_operand(
                    opcode,
                    mode_prefix.as_deref(),
                    expr,
                    index_reg.as_deref(),
                    *is_indirect,
                );
            }
            Operand::RegisterRef { name: _ } => {
                // cpu::a, cpu::x, cpu::y — for switch statements.
                // No direct encoding; these are handled by switch.
            }
            Operand::LabelRef { name } => {
                // Branch to label.
                if let Some(op_byte) = self.lookup(opcode, AddrMode::Relative) {
                    self.emit_byte(op_byte);
                    self.emit_byte(0); // placeholder offset
                    self.add_relocation(1, RelocKind::Branch8, name, 0);
                } else if let Some(op_byte) = self.lookup(opcode, AddrMode::Absolute) {
                    // Non-branch instruction with label ref — treat as absolute.
                    self.emit_byte(op_byte);
                    self.emit_byte(0);
                    self.emit_byte(0);
                    self.add_relocation(2, RelocKind::Abs16, name, 0);
                } else {
                    // The mnemonic has no relative or absolute encoding for
                    // this CPU. Silently dropping the instruction here used
                    // to corrupt the generated code, so report an error.
                    self.error(
                        301,
                        format!(
                            "opcode '{}' with a label operand is not supported on {}",
                            opcode, self.target.cpu
                        ),
                    );
                }
            }
            Operand::Selector { path, accesses } => {
                // Selector like PPU::CNT0 — resolve to a constant, or
                // fall back to a symbol relocation.
                let path_name = path.join("::");
                match resolve_selector(path, accesses, &self.const_values, &self.symbol_types) {
                    Some(val) => {
                        // Known constant: emit the (offset-adjusted)
                        // value directly as an absolute operand.
                        if let Some(op_byte) = self.lookup(opcode, AddrMode::Absolute) {
                            self.emit_byte(op_byte);
                            self.emit_bytes(&val_to_bytes(val, 2));
                        }
                    }
                    None if !path_name.is_empty() => {
                        // Not a known constant: keep the old behaviour
                        // and emit a relocation against the joined
                        // path, carrying the folded offset as the
                        // relocation addend.
                        if let Some(op_byte) = self.lookup(opcode, AddrMode::Absolute) {
                            self.emit_byte(op_byte);
                            self.emit_byte(0);
                            self.emit_byte(0);
                            self.add_relocation(
                                2,
                                RelocKind::Abs16,
                                &path_name,
                                selector_offset(accesses, &self.const_values, &self.symbol_types),
                            );
                        }
                    }
                    None => {}
                }
            }
        }
    }

    /// Emit a statement resolved through the official SM83 matrix:
    /// the matrix bytes first, then the dynamic operand role.
    fn compile_resolved_sm83(
        &mut self,
        resolved: crate::sm83_official::Resolved,
        operands: &[Operand],
    ) {
        for byte in &resolved.bytes {
            self.emit_byte(*byte);
        }
        let Some((index, role)) = resolved.trailing else {
            return;
        };
        let Some(operand) = operands.get(index) else {
            self.error(
                311,
                format!(
                    "official SM83 statement '{}': operand index out of range",
                    resolved.form
                ),
            );
            return;
        };
        match role {
            crate::sm83_official::Trailing::Value8 => {
                self.emit_sm83_value8(operand, &resolved.form);
            }
            crate::sm83_official::Trailing::Value16 => {
                self.emit_sm83_value16(operand, &resolved.form);
            }
            crate::sm83_official::Trailing::Branch8 => {
                if let Operand::LabelRef { name } = operand {
                    self.emit_byte(0); // placeholder displacement
                    self.add_relocation(1, RelocKind::Branch8, name, 0);
                } else {
                    self.error(
                        311,
                        format!(
                            "official SM83 statement '{}' needs a label operand \
                             for the relative branch",
                            resolved.form
                        ),
                    );
                }
            }
        }
    }

    /// Emit one little-endian byte for a resolved statement's value
    /// operand: a folded constant, or a byte-sized relocation against
    /// its symbol. This mirrors the immediate-operand path of the
    /// legacy encoding.
    fn emit_sm83_value8(&mut self, operand: &Operand, form: &str) {
        if let Operand::LabelRef { .. } = operand {
            self.error(
                311,
                format!("official SM83 statement '{form}': labels have no byte value"),
            );
            return;
        }
        let Some(expr) = operand_expr(operand) else {
            self.error(
                311,
                format!("official SM83 statement '{form}' needs a value operand"),
            );
            return;
        };
        match eval_expr(expr, &self.const_values, &self.symbol_types) {
            Some(value) => self.emit_byte((value & 0xFF) as u8),
            None => match self.classify_immediate(expr) {
                Some((sym, kind, addend)) => {
                    self.emit_byte(0);
                    self.add_relocation(1, kind, &sym, addend);
                }
                None => self.error(
                    311,
                    format!(
                        "official SM83 statement '{form}': the operand is \
                         neither a constant nor a symbol"
                    ),
                ),
            },
        }
    }

    /// Emit two little-endian bytes for a resolved statement's value
    /// operand: a folded constant, or a 16-bit absolute relocation
    /// against its symbol. A label target relocates against its name.
    fn emit_sm83_value16(&mut self, operand: &Operand, form: &str) {
        if let Operand::LabelRef { name } = operand {
            self.emit_byte(0);
            self.emit_byte(0);
            self.add_relocation(2, RelocKind::Abs16, name, 0);
            return;
        }
        let Some(expr) = operand_expr(operand) else {
            self.error(
                311,
                format!("official SM83 statement '{form}' needs a value operand"),
            );
            return;
        };
        match eval_expr(expr, &self.const_values, &self.symbol_types) {
            Some(value) => {
                self.emit_byte((value & 0xFF) as u8);
                self.emit_byte(((value >> 8) & 0xFF) as u8);
            }
            None => match self.classify_immediate(expr) {
                Some((symbol, _kind, addend)) => {
                    self.emit_byte(0);
                    self.emit_byte(0);
                    self.add_relocation(2, RelocKind::Abs16, &symbol, addend);
                }
                None => self.error(
                    311,
                    format!(
                        "official SM83 statement '{form}': the operand is \
                         neither a constant nor a symbol"
                    ),
                ),
            },
        }
    }

    fn compile_memory_operand(
        &mut self,
        opcode: &str,
        mode_prefix: Option<&str>,
        expr: &Expr,
        index_reg: Option<&str>,
        is_indirect: bool,
    ) {
        let val = eval_expr(expr, &self.const_values, &self.symbol_types);

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
        } else if is_indirect {
            // Parenthesized operand: (expr) or (expr), reg.
            // Indirect (JMP) or indirect-indexed (LDA/STA etc. with Y).
            if let Some(idx) = index_reg {
                if idx.contains("x") {
                    AddrMode::IndirectX
                } else {
                    AddrMode::IndirectY
                }
            } else {
                AddrMode::Indirect
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
            // LDH instructions use a 1-byte high-page address, not a
            // 2-byte absolute address.
            let is_one_byte_addr = opcode == "ldh";
            match val {
                Some(v) => {
                    if is_one_byte_addr
                        || mode == AddrMode::ZeroPage
                        || mode == AddrMode::ZeroPageX
                        || mode == AddrMode::ZeroPageY
                        || mode == AddrMode::IndirectX
                        || mode == AddrMode::IndirectY
                        || mode == AddrMode::Relative
                    {
                        self.emit_byte((v & 0xFF) as u8);
                    } else {
                        // Absolute or Indirect: 2 bytes, little-endian.
                        self.emit_byte((v & 0xFF) as u8);
                        self.emit_byte(((v >> 8) & 0xFF) as u8);
                    }
                }
                None => {
                    // Symbol reference, or an error if the expression is
                    // neither a constant nor a symbol.
                    let addend = selector_addend(expr, &self.const_values, &self.symbol_types);
                    let zp = is_one_byte_addr
                        || mode == AddrMode::ZeroPage
                        || mode == AddrMode::ZeroPageX
                        || mode == AddrMode::ZeroPageY
                        || mode == AddrMode::IndirectX
                        || mode == AddrMode::IndirectY;
                    if zp {
                        self.emit_byte(0);
                    } else {
                        self.emit_byte(0);
                        self.emit_byte(0);
                    }
                    match expr_to_symbol(expr) {
                        Some(sym) => {
                            let size = if zp { 1 } else { 2 };
                            let kind = if zp {
                                RelocKind::Abs8
                            } else {
                                RelocKind::Abs16
                            };
                            self.add_relocation(size, kind, &sym, addend);
                        }
                        None => {
                            self.error(305, "address operand is neither a constant nor a symbol");
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

    /// The unconditional absolute jump opcode for the target CPU
    /// family: `JP nn` on SM83/Z80, `JMP absolute` on the 6502 family.
    fn unconditional_jump_op(&self) -> u8 {
        match self.target.cpu.as_str() {
            "sm83" | "z80" => 0xC3, // JP
            _ => 0x4C,              // JMP
        }
    }

    /// The return opcode for the target CPU family: `RET` on SM83/Z80,
    /// `RTS` on the 6502 family. Used by the implicit return in
    /// `compile_fn` and by an explicit `return` statement.
    fn return_op(&self) -> u8 {
        match self.target.cpu.as_str() {
            "sm83" | "z80" => 0xC9, // RET
            _ => 0x60,              // RTS
        }
    }

    /// The fn name to quote in a flow-control diagnostic. Statements
    /// compiled outside a fn body report `<top-level>`.
    fn diag_fn_name(&self) -> String {
        if self.current_fn.is_empty() {
            "<top-level>".to_string()
        } else {
            self.current_fn.clone()
        }
    }

    /// Report E308 when a statement-level branch displacement does not
    /// fit in the signed 8-bit offset (-128..=127) that the conditional
    /// branches on every supported CPU (and the SM83/Z80 `JR`) encode.
    /// Without the check the displacement silently wraps during the
    /// `u8` cast and branches into random code.
    fn check_branch_distance(&mut self, distance: i64, stmt_kind: &str) {
        if !(-128..=127).contains(&distance) {
            let fn_name = self.diag_fn_name();
            self.error(
                308,
                format!(
                    "branch distance {distance} exceeds the signed 8-bit \
                     range (-128..=127) for the {stmt_kind} statement in fn '{fn_name}'"
                ),
            );
        }
    }

    fn compile_if(
        &mut self,
        condition: &Condition,
        then_block: &[FnStmt],
        else_block: &Option<Vec<FnStmt>>,
    ) {
        // An empty then-block would leave the placeholder offset
        // pointing at degenerate code: the patch value 0 makes the
        // conditional branch jump to the next instruction and the
        // condition test is dead. Fail instead of emitting it.
        if then_block.is_empty() {
            let fn_name = self.diag_fn_name();
            self.error(
                309,
                format!(
                    "'if' statement in fn '{fn_name}' has an empty \
                     then-block; the conditional branch has no target"
                ),
            );
            return;
        }

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
            // Emit jump past the else-block with the CPU-family jump.
            let jump_op = self.unconditional_jump_op();
            self.emit_byte(jump_op);
            let else_jump_patch = self.current_data_len();
            self.emit_byte(0);
            self.emit_byte(0);

            // Patch the branch to skip over the then-block + JMP.
            let then_size = self.current_data_len() as i64 - patch_offset as i64 - 1;
            self.check_branch_distance(then_size, "if");
            if then_size >= 0 {
                self.patch_byte(patch_offset, then_size as u8);
            }

            // Compile the else-block.
            for stmt in else_blk {
                self.compile_stmt(stmt);
            }

            // Patch the JMP to skip over the else-block.
            let else_end = self.current_data_len() as u32 + self.current_org();
            self.patch_byte(else_jump_patch, (else_end & 0xFF) as u8);
            self.patch_byte(else_jump_patch + 1, ((else_end >> 8) & 0xFF) as u8);
        } else {
            // Patch the branch to skip over the then-block.
            let then_size = self.current_data_len() as i64 - patch_offset as i64 - 1;
            self.check_branch_distance(then_size, "if");
            if then_size >= 0 {
                self.patch_byte(patch_offset, then_size as u8);
            }
        }
    }

    fn compile_while(&mut self, condition: &Condition, body: &[FnStmt]) {
        let loop_start = self.current_data_len() as u32;
        let org = self.current_org();
        let loop_abs = loop_start + org;

        // Emit branch-if-not-condition past the body.
        let branch_op = self.condition_to_branch_op(condition, false);
        self.emit_byte(branch_op);
        let patch_offset = self.current_data_len();
        self.emit_byte(0); // placeholder

        // Compile the body.
        for stmt in body {
            self.compile_stmt(stmt);
        }

        // Emit the unconditional back-jump to loop_start with the
        // CPU-family jump opcode (absolute address = offset + org).
        let jump_op = self.unconditional_jump_op();
        self.emit_byte(jump_op);
        self.emit_byte((loop_abs & 0xFF) as u8);
        self.emit_byte(((loop_abs >> 8) & 0xFF) as u8);

        // Patch the branch to skip over the body + jump.
        let body_size = self.current_data_len() as i64 - patch_offset as i64 - 1;
        self.check_branch_distance(body_size, "while");
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
        self.check_branch_distance(offset, "do-while");
        self.emit_byte(offset as u8);
    }

    fn compile_loop(&mut self, body: &[FnStmt]) {
        let loop_start = self.current_data_len() as u32;
        let org = self.current_org();
        let loop_abs = loop_start + org;

        // Compile the body.
        for stmt in body {
            self.compile_stmt(stmt);
        }

        // Emit JMP/JP back to loop_start (absolute address = offset + org).
        // Use the CPU-family-specific jump opcode.
        let jmp_opcode = self.unconditional_jump_op();
        self.emit_byte(jmp_opcode);
        self.emit_byte((loop_abs & 0xFF) as u8);
        self.emit_byte(((loop_abs >> 8) & 0xFF) as u8);
    }

    fn compile_switch(&mut self, _register: &str, cases: &[SwitchCase]) {
        // The switch lowering emits CMP #imm / BEQ pairs, which are
        // 6502-family encodings. Other CPU families have no switch
        // lowering yet: fail instead of emitting wrong bytes.
        if !crate::optimizer::is_6502_family_cpu(&self.target.cpu) {
            let fn_name = self.diag_fn_name();
            let cpu = self.target.cpu.clone();
            self.error(
                310,
                format!("'switch' statement in fn '{fn_name}' is not supported on CPU '{cpu}'"),
            );
            return;
        }
        // For each case, emit CMP #value then BEQ to the case body.
        let mut case_patches: Vec<(usize, u32)> = Vec::new();

        for case in cases {
            match case {
                SwitchCase::Case { expr, body: _ } => {
                    // CMP #value
                    let val = eval_expr(expr, &self.const_values, &self.symbol_types).unwrap_or(0);
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

    fn compile_fn_call(&mut self, name: &str, args: &[Expr]) {
        // NES runtime font load mapper guard: check that the mapper
        // supports CHR-RAM before expanding _font_load_tiles.
        if name == "_font_load_tiles" && self.target.machine == "nes" {
            if let Some(ref header) = self.header {
                if let Some(mapper_str) = header
                    .fields
                    .iter()
                    .find(|(k, _)| k == "mapper")
                    .map(|(_, v)| v.clone())
                {
                    let mapper_num: u32 = mapper_str.parse().unwrap_or(0);
                    const NON_CHR_RAM_MAPPERS: &[u32] = &[0, 2, 3, 6, 9, 10, 71];
                    if NON_CHR_RAM_MAPPERS.contains(&mapper_num) {
                        self.error(
                            307,
                            "runtime font_load() requires a CHR-RAM mapper on NES; \
                             use font_load! in a #[chr] block for CHR-ROM mappers",
                        );
                        return;
                    }
                }
            }
        }
        // Check if it's an inline fn.
        if let Some(inline_fn) = self.inline_fns.get(name).cloned() {
            if inline_fn.is_inline {
                // Substitute the call arguments for the parameters, then
                // expand the body at the call site. Nested inline calls
                // in the substituted body resolve during this compile
                // pass.
                let body = self.substitute_params(&inline_fn.body, &inline_fn.params, args);
                for stmt in &body {
                    self.compile_stmt(stmt);
                }
            } else {
                // Non-inline fn call: emit CALL/JSR plus an Abs16
                // relocation against the fn name. The fn body is placed
                // exactly once (in a section block),
                // so calls jump to it rather than inlining.
                let call_opcode = match self.target.cpu.as_str() {
                    "sm83" | "z80" => 0xCD, // CALL nn
                    _ => 0x20,              // JSR absolute (6502)
                };
                self.emit_byte(call_opcode);
                self.emit_byte(0);
                self.emit_byte(0);
                self.add_relocation(2, RelocKind::Abs16, name, 0);
            }
        } else {
            // Unknown fn call — emit CALL/JSR with a relocation; the
            // linker reports an unresolved symbol if nothing defines it.
            let call_opcode = match self.target.cpu.as_str() {
                "sm83" | "z80" => 0xCD, // CALL nn
                _ => 0x20,              // JSR absolute (6502)
            };
            self.emit_byte(call_opcode);
            self.emit_byte(0);
            self.emit_byte(0);
            self.add_relocation(2, RelocKind::Abs16, name, 0);
        }
    }

    // --- Parameter substitution ---------------------------------------------

    /// Clone an inline fn body, replacing parameter idents with the
    /// call-site argument expressions. Parameters without a matching
    /// argument are left as-is so the compiler falls back to the
    /// existing symbol handling for them.
    fn substitute_params(&self, body: &[FnStmt], params: &[String], args: &[Expr]) -> Vec<FnStmt> {
        body.iter()
            .map(|stmt| self.substitute_stmt(stmt, params, args))
            .collect()
    }

    fn substitute_stmt(&self, stmt: &FnStmt, params: &[String], args: &[Expr]) -> FnStmt {
        match stmt {
            FnStmt::AsmStmt { opcode, operands } => FnStmt::AsmStmt {
                opcode: opcode.clone(),
                operands: operands
                    .iter()
                    .map(|operand| self.substitute_operand(operand, params, args))
                    .collect(),
            },
            FnStmt::FnCall {
                name,
                args: call_args,
            } => FnStmt::FnCall {
                name: name.clone(),
                args: call_args
                    .iter()
                    .map(|arg| self.substitute_expr(arg, params, args))
                    .collect(),
            },
            FnStmt::Label { name, stmt } => FnStmt::Label {
                name: name.clone(),
                stmt: Box::new(self.substitute_stmt(stmt, params, args)),
            },
            FnStmt::IfStmt {
                branch_hint,
                condition,
                then_block,
                else_block,
            } => FnStmt::IfStmt {
                branch_hint: *branch_hint,
                condition: condition.clone(),
                then_block: self.substitute_block(then_block, params, args),
                else_block: else_block
                    .as_ref()
                    .map(|block| self.substitute_block(block, params, args)),
            },
            FnStmt::WhileStmt {
                branch_hint,
                condition,
                body,
            } => FnStmt::WhileStmt {
                branch_hint: *branch_hint,
                condition: condition.clone(),
                body: self.substitute_block(body, params, args),
            },
            FnStmt::DoWhileStmt {
                body,
                branch_hint,
                condition,
            } => FnStmt::DoWhileStmt {
                body: self.substitute_block(body, params, args),
                branch_hint: *branch_hint,
                condition: condition.clone(),
            },
            FnStmt::LoopStmt { body } => FnStmt::LoopStmt {
                body: self.substitute_block(body, params, args),
            },
            FnStmt::SwitchStmt { register, cases } => FnStmt::SwitchStmt {
                register: register.clone(),
                cases: cases
                    .iter()
                    .map(|case| self.substitute_case(case, params, args))
                    .collect(),
            },
            FnStmt::VarDeclStmt { decl } => FnStmt::VarDeclStmt {
                decl: Box::new(self.substitute_item(decl, params, args)),
            },
            FnStmt::ReturnStmt => FnStmt::ReturnStmt,
        }
    }

    fn substitute_block(&self, block: &[FnStmt], params: &[String], args: &[Expr]) -> Vec<FnStmt> {
        block
            .iter()
            .map(|stmt| self.substitute_stmt(stmt, params, args))
            .collect()
    }

    fn substitute_case(&self, case: &SwitchCase, params: &[String], args: &[Expr]) -> SwitchCase {
        match case {
            SwitchCase::Case { expr, body } => SwitchCase::Case {
                expr: self.substitute_expr(expr, params, args),
                body: self.substitute_block(body, params, args),
            },
            SwitchCase::Default { body } => SwitchCase::Default {
                body: self.substitute_block(body, params, args),
            },
        }
    }

    fn substitute_item(&self, item: &Item, params: &[String], args: &[Expr]) -> Item {
        match item {
            Item::ConstDecl {
                name,
                ty,
                value,
                evaluated_value,
                attributes,
            } => Item::ConstDecl {
                name: name.clone(),
                ty: ty.clone(),
                value: self.substitute_expr(value, params, args),
                evaluated_value: *evaluated_value,
                attributes: attributes.clone(),
            },
            Item::VarDecl {
                name,
                is_volatile,
                ty,
                array_dim,
                addr_binding,
                init,
                attributes,
            } => Item::VarDecl {
                name: name.clone(),
                is_volatile: *is_volatile,
                ty: ty.clone(),
                array_dim: array_dim
                    .as_ref()
                    .map(|dim| self.substitute_expr(dim, params, args)),
                addr_binding: addr_binding
                    .as_ref()
                    .map(|binding| self.substitute_expr(binding, params, args)),
                init: init
                    .as_ref()
                    .map(|init| self.substitute_init(init, params, args)),
                attributes: attributes.clone(),
            },
            _ => item.clone(),
        }
    }

    fn substitute_init(&self, init: &InitValue, params: &[String], args: &[Expr]) -> InitValue {
        match init {
            InitValue::Expr { value } => InitValue::Expr {
                value: self.substitute_expr(value, params, args),
            },
            InitValue::InitList { items } => InitValue::InitList {
                items: items
                    .iter()
                    .map(|item| self.substitute_init(item, params, args))
                    .collect(),
            },
            InitValue::String_ { value } => InitValue::String_ {
                value: value.clone(),
            },
        }
    }

    fn substitute_operand(&self, operand: &Operand, params: &[String], args: &[Expr]) -> Operand {
        match operand {
            Operand::Immediate { value } => Operand::Immediate {
                value: self.substitute_expr(value, params, args),
            },
            Operand::MemoryOperand {
                mode_prefix,
                expr,
                index_reg,
                is_indirect,
            } => Operand::MemoryOperand {
                mode_prefix: mode_prefix.clone(),
                expr: self.substitute_expr(expr, params, args),
                index_reg: index_reg.clone(),
                is_indirect: *is_indirect,
            },
            Operand::RegisterRef { name } => Operand::RegisterRef { name: name.clone() },
            Operand::LabelRef { name } => Operand::LabelRef { name: name.clone() },
            Operand::Selector { path, accesses } => Operand::Selector {
                path: path.clone(),
                accesses: accesses
                    .iter()
                    .map(|access| self.substitute_access(access, params, args))
                    .collect(),
            },
        }
    }

    fn substitute_access(&self, access: &Access, params: &[String], args: &[Expr]) -> Access {
        match access {
            Access::ModuleAccess { name } => Access::ModuleAccess { name: name.clone() },
            Access::FieldAccess { name } => Access::FieldAccess { name: name.clone() },
            Access::Offset { op, value } => Access::Offset {
                op: *op,
                value: self.substitute_expr(value, params, args),
            },
        }
    }

    /// Replace `Expr::Ident` names that match a parameter with the
    /// matching argument expression, recursing into composite
    /// expressions.
    fn substitute_expr(&self, expr: &Expr, params: &[String], args: &[Expr]) -> Expr {
        match expr {
            Expr::Ident { name } => params
                .iter()
                .position(|param| param == name)
                .and_then(|index| args.get(index))
                .cloned()
                .unwrap_or_else(|| expr.clone()),
            Expr::BinOp { op, left, right } => Expr::BinOp {
                op: *op,
                left: Box::new(self.substitute_expr(left, params, args)),
                right: Box::new(self.substitute_expr(right, params, args)),
            },
            Expr::UnaryOp { op, operand } => Expr::UnaryOp {
                op: *op,
                operand: Box::new(self.substitute_expr(operand, params, args)),
            },
            Expr::MacroCall { name, arg } => Expr::MacroCall {
                name: name.clone(),
                arg: Box::new(self.substitute_expr(arg, params, args)),
            },
            Expr::Selector { path, accesses } => {
                // A path element can be a parameter: `dest + 1`
                // parses as a selector on `dest`. An identifier
                // argument replaces the name in place; other argument
                // shapes leave the selector unchanged.
                let path = path
                    .iter()
                    .map(|segment| {
                        params
                            .iter()
                            .position(|param| param == segment)
                            .and_then(|index| args.get(index))
                            .and_then(|arg| match arg {
                                Expr::Ident { name } => Some(name.clone()),
                                _ => None,
                            })
                            .unwrap_or_else(|| segment.clone())
                    })
                    .collect();
                Expr::Selector {
                    path,
                    accesses: accesses
                        .iter()
                        .map(|access| self.substitute_access(access, params, args))
                        .collect(),
                }
            }
            Expr::FnCall {
                name,
                args: call_args,
            } => Expr::FnCall {
                name: name.clone(),
                args: call_args
                    .iter()
                    .map(|arg| self.substitute_expr(arg, params, args))
                    .collect(),
            },
            Expr::ParenExpr { inner } => Expr::ParenExpr {
                inner: Box::new(self.substitute_expr(inner, params, args)),
            },
            Expr::Number { .. } | Expr::String_ { .. } | Expr::Boolean { .. } => expr.clone(),
            Expr::ArrayLit { elements } => Expr::ArrayLit {
                elements: elements
                    .iter()
                    .map(|e| self.substitute_expr(e, params, args))
                    .collect(),
            },
            Expr::StructLit { type_name, fields } => Expr::StructLit {
                type_name: type_name.clone(),
                fields: fields
                    .iter()
                    .map(|(n, e)| (n.clone(), self.substitute_expr(e, params, args)))
                    .collect(),
            },
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

    /// Return the `org` address of the current section, or 0 if no section
    /// is active. JMP absolute targets must add this to the section-relative
    /// offset to form the correct absolute address.
    fn current_org(&self) -> u32 {
        if let Some(idx) = self.current_section {
            self.sections[idx].org
        } else {
            0
        }
    }

    fn add_relocation(&mut self, offset_from_end: u32, kind: RelocKind, symbol: &str, addend: i64) {
        if let Some(idx) = self.current_section {
            let abs_offset = self.sections[idx].data.len() as u32 - offset_from_end;
            self.sections[idx].relocations.push(Relocation {
                offset: abs_offset,
                kind,
                symbol: symbol.to_string(),
                addend,
            });
        }
    }

    fn error(&mut self, code: u32, msg: impl Into<String>) {
        self.diags.push(Diagnostic::error(code, "", 0, 0, msg));
    }

    fn warning(&mut self, code: u32, msg: impl Into<String>) {
        self.diags.push(Diagnostic::warning(code, "", 0, 0, msg));
    }

    /// Map a condition keyword to a branch opcode byte.
    /// If `invert` is true, return the branch-if-condition opcode.
    /// If `invert` is false, return the branch-if-not-condition opcode.
    fn condition_to_branch_op(&self, condition: &Condition, invert: bool) -> u8 {
        // Condition keywords to branch opcodes. The opcode depends on
        // the CPU family.
        let (branch_if_true, branch_if_false) = match self.target.cpu.as_str() {
            // SM83 / Z80: JR NZ=0x20, JR Z=0x28, JR NC=0x30, JR C=0x38
            "sm83" | "z80" => match condition.keyword.as_str() {
                "carry" => (0x38, 0x30),                    // JR C, JR NC
                "nonzero" | "set" | "true" => (0x20, 0x28), // JR NZ, JR Z
                "zero" | "unset" | "false" | "clear" | "equal" => (0x28, 0x20), // JR Z, JR NZ
                _ => (0x20, 0x28),                          // default: JR NZ, JR Z
            },
            // 6502 family: BPL=0x10, BMI=0x30, BVS=0x70, BVC=0x50, etc.
            _ => match condition.keyword.as_str() {
                "plus" | "positive" | "greater" => (0x10, 0x30), // BPL, BMI
                "minus" | "negative" | "less" => (0x30, 0x10),   // BMI, BPL
                "overflow" => (0x70, 0x50),                      // BVS, BVC
                "carry" => (0xB0, 0x90),                         // BCS, BCC
                "nonzero" | "set" | "true" => (0xD0, 0xF0),      // BNE, BEQ
                "zero" | "unset" | "false" | "clear" | "equal" => (0xF0, 0xD0), // BEQ, BNE
                _ => (0xD0, 0xF0),                               // default: BNE, BEQ
            },
        };
        // The "not" modifier inverts the condition sense.
        let inverted_by_mod = condition.modifiers.iter().any(|m| m == "not");
        let effective_invert = invert ^ inverted_by_mod;
        if effective_invert {
            branch_if_true
        } else {
            branch_if_false
        }
    }
}

// --- Helper functions -------------------------------------------------------

/// Look up the vector table address for an interrupt name on a given CPU family.
/// Returns the address where the linker should write the vector entry.
pub fn interrupt_vector_address(cpu: &str, interrupt_name: &str) -> Option<u32> {
    match cpu {
        "mos6502" | "mos65sc02" | "w65c02" | "rp2A03" | "rp2A07" | "vl65NC02" => {
            match interrupt_name {
                "reset" => Some(0xFFFC),
                "nmi" => Some(0xFFFA),
                "irq" => Some(0xFFF8),
                _ => None,
            }
        }
        "wdc65c816" => match interrupt_name {
            "reset" => Some(0xFFFC),
            "nmi" => Some(0xFFEA),
            "irq" => Some(0xFFEE),
            "abort" => Some(0xFFE8),
            "cop" => Some(0xFFE4),
            "brk" => Some(0xFFE6),
            _ => None,
        },
        "sm83" => match interrupt_name {
            "reset" => Some(0x0100),
            "vblank" => Some(0x0040),
            "lcdc" => Some(0x0048),
            "timer" => Some(0x0050),
            "serial" => Some(0x0058),
            "joypad" => Some(0x0060),
            _ => None,
        },
        "z80" => match interrupt_name {
            "reset" => Some(0x0000),
            "rst8" => Some(0x0008),
            "rst10" => Some(0x0010),
            "rst18" => Some(0x0018),
            "rst20" => Some(0x0020),
            "rst28" => Some(0x0028),
            "rst30" => Some(0x0030),
            "rst38" | "irq" => Some(0x0038),
            "nmi" => Some(0x0066),
            _ => None,
        },
        "m68000" => {
            // Explicit exception vectors first.
            match interrupt_name {
                "reset" => return Some(0x0000),
                "reset_pc" => return Some(0x0004),
                "bus_error" => return Some(0x0008),
                "address_error" => return Some(0x000C),
                "illegal" => return Some(0x0010),
                "zero_divide" => return Some(0x0014),
                "chk" => return Some(0x0018),
                "trapv" => return Some(0x001C),
                "privilege" => return Some(0x0020),
                "trace" => return Some(0x0024),
                "line_a" => return Some(0x0028),
                "line_f" => return Some(0x002C),
                "spurious" => return Some(0x0060),
                "level1" => return Some(0x0064),
                "level2" => return Some(0x0068),
                "level3" => return Some(0x006C),
                "level4" => return Some(0x0070),
                "level5" => return Some(0x0074),
                "level6" => return Some(0x0078),
                "level7" => return Some(0x007C),
                _ => {}
            }
            // Trap vectors are 0x0080 + n*4 for trap0 through trap15.
            if let Some(rest) = interrupt_name.strip_prefix("trap") {
                if let Ok(n) = rest.parse::<u32>() {
                    if n <= 15 {
                        return Some(0x0080 + n * 4);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// Get the vector encoding for a CPU family.
pub fn vector_encoding_for(cpu: &str) -> op_ir::VectorEncoding {
    match cpu {
        "m68000" => op_ir::VectorEncoding::Pointer4,
        "z80" => op_ir::VectorEncoding::JumpZ80,
        "sm83" => op_ir::VectorEncoding::JumpSm83,
        _ => op_ir::VectorEncoding::Pointer2,
    }
}

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

/// Section name prefix for a section kind.
fn kind_name(kind: SectionKind) -> &'static str {
    match kind {
        SectionKind::Rom => "rom",
        SectionKind::Chr => "chr",
        SectionKind::Ram => "ram",
    }
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

/// Resolve a selector (`path::access + offset`) to a constant value.
///
/// The fully qualified name (e.g. `PPU::CNT0`) is tried first, then the
/// bare path (`PPU`). Any trailing offset access is folded into the
/// result. Returns `None` when the selector does not name a constant.
/// Fold the evaluable `Offset` accesses of a selector into a signed
/// addend. Offsets that cannot be evaluated are ignored.
fn selector_offset(
    accesses: &[Access],
    const_values: &HashMap<String, i64>,
    symbol_types: &HashMap<String, Type>,
) -> i64 {
    let mut offset: i64 = 0;
    for access in accesses {
        if let Access::Offset { op, value } = access {
            if let Some(v) = eval_expr(value, const_values, symbol_types) {
                offset = match op {
                    OffsetOp::Add => offset + v,
                    OffsetOp::Sub => offset - v,
                };
            }
        }
    }
    offset
}

/// The offset addend of an expression, if it is a selector with
/// offset accesses; otherwise zero.
fn selector_addend(
    expr: &Expr,
    const_values: &HashMap<String, i64>,
    symbol_types: &HashMap<String, Type>,
) -> i64 {
    match expr {
        Expr::Selector { accesses, .. } => selector_offset(accesses, const_values, symbol_types),
        _ => 0,
    }
}

fn resolve_selector(
    path: &[String],
    accesses: &[Access],
    const_values: &HashMap<String, i64>,
    symbol_types: &HashMap<String, Type>,
) -> Option<i64> {
    let path_name = path.join("::");
    let mut full_name = path_name.clone();
    for access in accesses {
        if let Access::ModuleAccess { name } | Access::FieldAccess { name } = access {
            full_name.push_str("::");
            full_name.push_str(name);
        }
    }
    let offset = selector_offset(accesses, const_values, symbol_types);
    let value = const_values.get(&full_name).or_else(|| {
        if full_name != path_name {
            const_values.get(&path_name)
        } else {
            None
        }
    })?;
    Some(value + offset)
}

/// Evaluate an expression to a constant value, using the const value table.
fn eval_expr(
    expr: &Expr,
    const_values: &HashMap<String, i64>,
    symbol_types: &HashMap<String, Type>,
) -> Option<i64> {
    match expr {
        Expr::Number { value } => Some(*value),
        Expr::Boolean { value } => Some(if *value { 1 } else { 0 }),
        Expr::Ident { name } => const_values.get(name).copied(),
        Expr::UnaryOp { op, operand } => {
            let v = eval_expr(operand, const_values, symbol_types)?;
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
            let l = eval_expr(left, const_values, symbol_types)?;
            let r = eval_expr(right, const_values, symbol_types)?;
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
        Expr::MacroCall { name, arg } => match name.as_str() {
            "lo" | "hi" | "nylo" | "nyhi" => {
                let v = eval_expr(arg, const_values, symbol_types)?;
                Some(match name.as_str() {
                    "lo" => v & 0xFF,
                    "hi" => (v >> 8) & 0xFF,
                    "nylo" => v & 0x0F,
                    "nyhi" => (v >> 4) & 0x0F,
                    _ => return None,
                })
            }
            "len" => {
                if let Expr::Ident { name: sym } = arg.as_ref() {
                    if let Some(Type::Array {
                        size: Some(size_expr),
                        ..
                    }) = symbol_types.get(sym)
                    {
                        eval_expr(size_expr, const_values, symbol_types)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            "sizeof" => {
                if let Expr::Ident { name: sym } = arg.as_ref() {
                    symbol_types.get(sym).map(|ty| type_size(ty) as i64)
                } else {
                    None
                }
            }
            _ => None,
        },
        Expr::ParenExpr { inner } => eval_expr(inner, const_values, symbol_types),
        // Selector like PPU::CNT0 — resolve against the const table so
        // that memory and immediate operands can use the value directly.
        Expr::Selector { path, accesses } => {
            resolve_selector(path, accesses, const_values, symbol_types)
        }
        Expr::ArrayLit { .. } => None,
        Expr::StructLit { .. } => None,
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

/// The value expression of an assembly operand: the expression after
/// `#` for an immediate, or the parenthesized address expression for
/// a memory operand. A label operand carries a name instead of an
/// expression.
fn operand_expr(operand: &Operand) -> Option<&Expr> {
    match operand {
        Operand::Immediate { value } => Some(value),
        Operand::MemoryOperand { expr, .. } => Some(expr),
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
    let size = size.min(8); // Cap at 8 bytes (i64 max)
    let mut bytes = Vec::with_capacity(size);
    for i in 0..size {
        bytes.push(((val >> (i * 8)) & 0xFF) as u8);
    }
    bytes
}

/// Find the std crate root directory.
///
/// Searches, in order:
/// 1. The CLI include paths (`-I` / `--include`), in the order given.
/// 2. The `OP_STD_PATH` environment variable.
/// 3. The default install path `$HOME/.carts/std/src`.
///
/// Returns the first candidate directory that contains a `lib.op` file.
fn find_std_root(include_paths: &[String]) -> Option<std::path::PathBuf> {
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();

    for path in include_paths {
        candidates.push(std::path::Path::new(path).to_path_buf());
    }

    if let Ok(env) = std::env::var("OP_STD_PATH") {
        if !env.is_empty() {
            candidates.push(std::path::Path::new(&env).to_path_buf());
        }
    }

    if let Some(home) = std::env::var_os("HOME") {
        candidates.push(std::path::Path::new(&home).join(".carts/std/src"));
    }

    candidates
        .into_iter()
        .find(|candidate| candidate.join("lib.op").is_file())
}

/// Map a module path to its source file. A module with segments
/// `a/b/c` lives in `dir/a/b/c.op` or `dir/a/b/c/mod.op`; a module
/// with no segments lives in `dir/lib.op` or `dir/mod.op`.
fn module_file_path(dir: &std::path::Path, segments: &[String]) -> Option<std::path::PathBuf> {
    if segments.is_empty() {
        for name in ["lib.op", "mod.op"] {
            let file = dir.join(name);
            if file.is_file() {
                return Some(file);
            }
        }
        return None;
    }
    let mut base = dir.to_path_buf();
    for segment in &segments[..segments.len() - 1] {
        base.push(segment);
    }
    base.push(&segments[segments.len() - 1]);
    let name = base.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let file = base.with_file_name(format!("{name}.op"));
    if file.is_file() {
        return Some(file);
    }
    let mod_file = base.join("mod.op");
    if mod_file.is_file() {
        return Some(mod_file);
    }
    None
}

/// Return the name a declaration binds, if any.
fn decl_name(item: &Item) -> Option<&str> {
    match item {
        Item::ConstDecl { name, .. }
        | Item::VarDecl { name, .. }
        | Item::FnDecl { name, .. }
        | Item::InlineFnDecl { name, .. }
        | Item::StructDecl { name, .. }
        | Item::TypeDecl { name, .. }
        | Item::EnumDecl { name, .. } => Some(name),
        Item::ModDecl { name, .. } => Some(name),
        Item::UseDecl { .. } | Item::BlockAttribute { .. } | Item::Placement { .. } => None,
    }
}

// --- Tests ------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::find_std_root;
    use super::Codegen;
    use super::InlineFn;
    use super::ModuleCache;
    use crate::encoding::get_full_encoding_table;
    use op_common::ast::{
        Access, EnumVariant, Expr, FnStmt, OffsetOp, Operand, UseRoot, UseTail, UseTree,
    };
    use op_common::TargetTriplet;
    use op_diagnostics::Severity;
    use op_ir::{RelocKind, Section, SectionKind, Symbol, SymbolKind};
    use std::collections::HashMap;

    /// Serializes tests that mutate the process environment.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Run `f` with `OP_STD_PATH` and `HOME` pointed at locations that do
    /// not contain a std root, then restore the previous environment.
    fn with_isolated_env(tmp: &std::path::Path, f: impl FnOnce()) {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let old_op_std = std::env::var("OP_STD_PATH").ok();
        let old_home = std::env::var("HOME").ok();
        std::env::set_var("OP_STD_PATH", tmp.join("no-such-std"));
        std::env::set_var("HOME", tmp);
        f();
        match old_op_std {
            Some(value) => std::env::set_var("OP_STD_PATH", value),
            None => std::env::remove_var("OP_STD_PATH"),
        }
        match old_home {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }
    }

    /// Run `f` with `OP_STD_PATH` set to `root` and `HOME` pointed at an
    /// empty directory, then restore the previous environment. Holds
    /// the environment lock for the duration.
    fn with_std_env(root: &std::path::Path, f: impl FnOnce()) {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let old_op_std = std::env::var("OP_STD_PATH").ok();
        let old_home = std::env::var("HOME").ok();
        std::env::set_var("OP_STD_PATH", root);
        std::env::set_var("HOME", std::env::temp_dir());
        f();
        match old_op_std {
            Some(value) => std::env::set_var("OP_STD_PATH", value),
            None => std::env::remove_var("OP_STD_PATH"),
        }
        match old_home {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }
    }

    /// Write a minimal fake std crate under `std_root`.
    fn write_fake_std(std_root: &std::path::Path) {
        std::fs::create_dir_all(std_root.join("cpu")).unwrap();
        std::fs::write(std_root.join("lib.op"), "pub mod cpu;\n").unwrap();
        std::fs::write(
            std_root.join("cpu.op"),
            "const CYCLES: u8 = 1;\nmod mos6502;\npub use mos6502::*;\n",
        )
        .unwrap();
        std::fs::write(
            std_root.join("cpu/mos6502.op"),
            "enum REGS { A = 0x2000, B = 0x2001 }\ninline fn nop() {\n    nop\n}\npub use REGS::*;\n",
        )
        .unwrap();
    }

    /// Write a fake std crate mirroring the `machine/nes` layout: a
    /// module whose private `use super::` imports bind the names its
    /// inline fns reference.
    fn write_nes_std(std_root: &std::path::Path) {
        std::fs::create_dir_all(std_root.join("nes")).unwrap();
        std::fs::write(std_root.join("lib.op"), "pub mod nes;\n").unwrap();
        std::fs::write(
            std_root.join("nes.op"),
            "mod constants;\nmod macros;\npub use constants::*;\npub use macros::*;\n",
        )
        .unwrap();
        std::fs::write(
            std_root.join("nes/constants.op"),
            "const ST_VBLANK: u8 = 0x80;\n",
        )
        .unwrap();
        std::fs::write(
            std_root.join("nes/macros.op"),
            "use super::constants::*;\ninline fn vblank_on() {\n    sta ST_VBLANK\n}\n",
        )
        .unwrap();
    }

    #[test]
    fn find_std_root_returns_none_when_missing() {
        let tmp = std::env::temp_dir().join(format!("opc-find-std-root-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        with_isolated_env(&tmp, || {
            let include = vec![tmp.join("no-such-include").to_string_lossy().to_string()];
            assert_eq!(find_std_root(&include), None);
            assert_eq!(find_std_root(&[]), None);
        });
    }

    #[test]
    fn load_module_caches_parsed_module() {
        let tmp = std::env::temp_dir().join(format!("opc-module-cache-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let tmp = tmp.canonicalize().unwrap();
        let file = tmp.join("cache-test.op");
        std::fs::write(&file, "const ANSWER: u8 = 42;\n").unwrap();
        let target = TargetTriplet::parse("rp2A03-nintendo-nes-ntsc").unwrap();

        let mut cache = ModuleCache::default();
        let first = cache.load_module(&file, &target, &[]).unwrap();
        assert_eq!(first.items.len(), 1);

        // Remove the file. A second load must still succeed from the cache.
        std::fs::remove_file(&file).unwrap();
        let second = cache.load_module(&file, &target, &[]).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn load_module_errors_when_file_missing() {
        let target = TargetTriplet::parse("rp2A03-nintendo-nes-ntsc").unwrap();
        let mut cache = ModuleCache::default();
        let missing = std::path::Path::new("/nonexistent-opc-test/missing.op");
        assert!(cache.load_module(missing, &target, &[]).is_err());
    }

    /// Build a minimal Codegen for unit tests.
    fn test_codegen() -> Codegen {
        let target = TargetTriplet::parse("rp2A03-nintendo-nes-ntsc").unwrap();
        Codegen {
            target,
            opt_level: 0,
            current_fn: String::new(),
            encoding_table: get_full_encoding_table("rp2A03"),
            sections: Vec::new(),
            current_section: None,
            inline_fns: HashMap::new(),
            const_values: HashMap::new(),
            const_arrays: HashMap::new(),
            struct_consts: HashMap::new(),
            collected_vars: Vec::new(),
            collected_consts: Vec::new(),
            symbol_types: HashMap::new(),
            module_cache: ModuleCache::default(),
            module_path: Vec::new(),
            enum_variants: HashMap::new(),
            use_aliases: HashMap::new(),
            include_paths: Vec::new(),
            features: Vec::new(),
            current_module_dir: None,
            label_counter: 0,
            interrupt_vectors: Vec::new(),
            header: None,
            pad_byte: 0,
            source_dir: std::path::PathBuf::new(),
            placed_items: std::collections::HashSet::new(),
            crash_handler_symbol: "__op_default_crash_handler".to_string(),
            struct_sizes: HashMap::new(),
            pending_locate: (None, None),
            fn_locate_pins: HashMap::new(),
            noreturn_fns: std::collections::HashSet::new(),
            collected_locates: HashMap::new(),
            diags: Vec::new(),
        }
    }

    #[test]
    fn resolve_root_converts_use_roots() {
        let mut codegen = test_codegen();

        // At the crate root: lib, self, and super all resolve to the
        // empty path.
        assert_eq!(codegen.resolve_root(&UseRoot::Lib), Vec::<String>::new());
        assert_eq!(
            codegen.resolve_root(&UseRoot::SelfMod),
            Vec::<String>::new()
        );
        assert_eq!(codegen.resolve_root(&UseRoot::Super), Vec::<String>::new());
        assert_eq!(
            codegen.resolve_root(&UseRoot::Name("std".into())),
            vec!["std".to_string()]
        );

        // Inside a nested module: self is the full stack, super drops
        // the last element.
        codegen.module_path.push("std".to_string());
        codegen.module_path.push("cpu".to_string());
        assert_eq!(
            codegen.current_module_path(),
            vec!["std".to_string(), "cpu".to_string()]
        );
        assert_eq!(
            codegen.resolve_root(&UseRoot::SelfMod),
            vec!["std".to_string(), "cpu".to_string()]
        );
        assert_eq!(
            codegen.resolve_root(&UseRoot::Super),
            vec!["std".to_string()]
        );
    }

    #[test]
    fn use_tree_resolves_std_items() {
        let tmp = std::env::temp_dir().join(format!("opc-use-tree-{}", std::process::id()));
        let std_root = tmp.join("std/src");
        write_fake_std(&std_root);

        with_std_env(&std_root, || {
            let mut codegen = test_codegen();
            let tree = UseTree::Path {
                root: UseRoot::Name("std".into()),
                segments: vec!["cpu".to_string()],
                tail: UseTail::Glob,
            };
            codegen.resolve_use_decl(&[tree]);

            assert_eq!(codegen.const_values.get("CYCLES"), Some(&1));
            assert_eq!(codegen.enum_variants.get("REGS::A"), Some(&0x2000));
            assert_eq!(codegen.const_values.get("REGS::A"), Some(&0x2000));
            // A glob of an enum also binds the variant names bare.
            assert_eq!(codegen.const_values.get("A"), Some(&0x2000));
            assert!(codegen.inline_fns.contains_key("nop"));
            assert!(codegen.diags.is_empty());
        });
    }

    #[test]
    fn use_tree_item_import_binds_single_item() {
        let tmp = std::env::temp_dir().join(format!("opc-use-tree-item-{}", std::process::id()));
        let std_root = tmp.join("std/src");
        write_fake_std(&std_root);

        with_std_env(&std_root, || {
            let mut codegen = test_codegen();
            let tree = UseTree::Path {
                root: UseRoot::Name("std".into()),
                segments: vec!["cpu".to_string(), "CYCLES".to_string()],
                tail: UseTail::Item,
            };
            codegen.resolve_use_decl(&[tree]);

            assert_eq!(codegen.const_values.get("CYCLES"), Some(&1));
            // An item import does not pull in the module's other items.
            assert!(!codegen.const_values.contains_key("A"));
            assert!(!codegen.inline_fns.contains_key("nop"));
            assert!(codegen.diags.is_empty());
        });
    }

    #[test]
    fn use_tree_records_module_aliases() {
        let mut codegen = test_codegen();
        let inner = UseTree::Path {
            root: UseRoot::Name("std".into()),
            segments: vec!["cpu".to_string()],
            tail: UseTail::Item,
        };
        let tree = UseTree::Alias {
            inner: Box::new(inner),
            alias: "c".to_string(),
        };
        codegen.resolve_use_decl(&[tree]);

        let expected = vec!["std".to_string(), "cpu".to_string()];
        assert_eq!(codegen.use_aliases.get("c"), Some(&expected));
    }

    #[test]
    fn use_tree_errors_when_std_missing() {
        let tmp = std::env::temp_dir().join(format!("opc-use-tree-nostd-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();

        with_isolated_env(&tmp, || {
            let mut codegen = test_codegen();
            let tree = UseTree::Path {
                root: UseRoot::Name("std".into()),
                segments: vec!["cpu".to_string()],
                tail: UseTail::Glob,
            };
            codegen.resolve_use_decl(&[tree]);

            assert!(codegen
                .diags
                .iter()
                .any(|d| d.severity == Severity::Error && d.code == 302));
            assert!(codegen.const_values.is_empty());
        });
    }

    /// Build a variant with an explicit numeric value.
    fn num_variant(name: &str, value: i64) -> EnumVariant {
        EnumVariant {
            name: name.to_string(),
            value: Some(Expr::Number { value }),
        }
    }

    /// Build a variant with no explicit value.
    fn implicit_variant(name: &str) -> EnumVariant {
        EnumVariant {
            name: name.to_string(),
            value: None,
        }
    }

    #[test]
    fn collect_enum_evaluates_explicit_and_implicit_values() {
        let mut codegen = test_codegen();

        // Explicit values are evaluated as written.
        codegen.collect_enum(
            "STATUS",
            &[
                num_variant("N", 0x80),
                num_variant("V", 0x40),
                num_variant("C", 0x01),
            ],
            false,
        );
        assert_eq!(codegen.const_values.get("STATUS::N"), Some(&0x80));
        assert_eq!(codegen.const_values.get("STATUS::V"), Some(&0x40));
        assert_eq!(codegen.const_values.get("STATUS::C"), Some(&0x01));
        assert_eq!(codegen.enum_variants.get("STATUS::N"), Some(&0x80));

        // Variants without a value count up from the previous variant,
        // starting at zero for the first variant.
        codegen.collect_enum(
            "OPCODE",
            &[
                implicit_variant("BRK"),
                implicit_variant("ORA"),
                implicit_variant("JMP"),
            ],
            false,
        );
        assert_eq!(codegen.const_values.get("OPCODE::BRK"), Some(&0));
        assert_eq!(codegen.const_values.get("OPCODE::ORA"), Some(&1));
        assert_eq!(codegen.const_values.get("OPCODE::JMP"), Some(&2));

        // An explicit value resets the implicit count.
        codegen.collect_enum(
            "COND",
            &[
                num_variant("plus", 0),
                implicit_variant("minus"),
                num_variant("equal", 5),
                implicit_variant("carry"),
            ],
            false,
        );
        assert_eq!(codegen.const_values.get("COND::plus"), Some(&0));
        assert_eq!(codegen.const_values.get("COND::minus"), Some(&1));
        assert_eq!(codegen.const_values.get("COND::equal"), Some(&5));
        assert_eq!(codegen.const_values.get("COND::carry"), Some(&6));

        // Glob import binds bare names, but never overwrites a name
        // that is already bound.
        codegen.const_values.insert("a".to_string(), 99);
        codegen.collect_enum(
            "REGS",
            &[implicit_variant("a"), implicit_variant("x")],
            true,
        );
        assert_eq!(codegen.const_values.get("REGS::a"), Some(&0));
        assert_eq!(codegen.const_values.get("a"), Some(&99));
        assert_eq!(codegen.const_values.get("x"), Some(&1));
        assert!(codegen.diags.is_empty());
    }

    /// Build a minimal Codegen with an active ROM section, so emitted
    /// instruction bytes and relocations can be inspected.
    fn test_codegen_with_rom_section() -> Codegen {
        let mut codegen = test_codegen();
        codegen.sections.push(Section {
            name: "rom_bank0".to_string(),
            kind: SectionKind::Rom,
            org: 0x8000,
            bank: 0,
            maxsize: 0x8000,
            symbols: Vec::new(),
            relocations: Vec::new(),
            data: Vec::new(),
        });
        codegen.current_section = Some(0);
        codegen
    }

    /// Build a selector operand from a `::`-separated name like
    /// `PPU::CNT0`.
    fn selector_operand(name: &str) -> Operand {
        let mut parts = name.split("::");
        let head = parts.next().unwrap_or_default().to_string();
        let accesses = parts
            .map(|part| Access::ModuleAccess {
                name: part.to_string(),
            })
            .collect();
        Operand::Selector {
            path: vec![head],
            accesses,
        }
    }

    #[test]
    fn selector_resolves_to_const_value() {
        let mut codegen = test_codegen_with_rom_section();
        codegen.const_values.insert("PPU::CNT0".to_string(), 0x2000);

        codegen.compile_asm("sta", &[selector_operand("PPU::CNT0")]);

        // `sta $2000` -> 8D 00 20, with no relocation.
        assert_eq!(codegen.sections[0].data, vec![0x8D, 0x00, 0x20]);
        assert!(codegen.sections[0].relocations.is_empty());
    }

    #[test]
    fn selector_falls_back_to_path_name() {
        let mut codegen = test_codegen_with_rom_section();
        // Only the bare path is a known constant.
        codegen.const_values.insert("PPU".to_string(), 0x2000);

        codegen.compile_asm("sta", &[selector_operand("PPU::CNT0")]);

        assert_eq!(codegen.sections[0].data, vec![0x8D, 0x00, 0x20]);
        assert!(codegen.sections[0].relocations.is_empty());
    }

    #[test]
    fn selector_emits_relocation_when_unknown() {
        let mut codegen = test_codegen_with_rom_section();

        codegen.compile_asm("sta", &[selector_operand("PPU::CNT0")]);

        // Placeholder bytes plus an Abs16 relocation against `PPU`.
        assert_eq!(codegen.sections[0].data, vec![0x8D, 0x00, 0x00]);
        assert_eq!(codegen.sections[0].relocations.len(), 1);
        assert_eq!(codegen.sections[0].relocations[0].symbol, "PPU");
    }

    /// The parser delivers selectors as `Expr::Selector` inside a
    /// memory operand, so verify that path resolves constants too.
    #[test]
    fn selector_in_memory_operand_resolves_to_const() {
        let mut codegen = test_codegen_with_rom_section();
        codegen.const_values.insert("PPU::CNT0".to_string(), 0x2000);

        let expr = Expr::Selector {
            path: vec!["PPU".to_string()],
            accesses: vec![Access::ModuleAccess {
                name: "CNT0".to_string(),
            }],
        };
        let operand = Operand::MemoryOperand {
            mode_prefix: None,
            expr,
            index_reg: None,
            is_indirect: false,
        };
        codegen.compile_asm("sta", &[operand]);

        // `sta $2000` -> 8D 00 20, with no relocation.
        assert_eq!(codegen.sections[0].data, vec![0x8D, 0x00, 0x20]);
        assert!(codegen.sections[0].relocations.is_empty());
    }

    /// A trailing offset access folds into the resolved constant.
    #[test]
    fn selector_with_offset_folds_into_value() {
        let mut codegen = test_codegen_with_rom_section();
        codegen.const_values.insert("PPU::CNT0".to_string(), 0x2000);

        let expr = Expr::Selector {
            path: vec!["PPU".to_string()],
            accesses: vec![
                Access::ModuleAccess {
                    name: "CNT0".to_string(),
                },
                Access::Offset {
                    op: OffsetOp::Add,
                    value: Expr::Number { value: 1 },
                },
            ],
        };
        let operand = Operand::MemoryOperand {
            mode_prefix: None,
            expr,
            index_reg: None,
            is_indirect: false,
        };
        codegen.compile_asm("sta", &[operand]);

        // `sta $2001` -> 8D 01 20.
        assert_eq!(codegen.sections[0].data, vec![0x8D, 0x01, 0x20]);
        assert!(codegen.sections[0].relocations.is_empty());
    }

    /// `lda #lo!(sym)` emits `A9 00` plus a one-byte `Lo8` relocation
    /// against `sym`. `lda #hi!(sym)` emits the same bytes with a `Hi8`
    /// relocation.
    #[test]
    fn immediate_lo_hi_of_symbol_emits_relocations() {
        let mk = |macro_name: &str, expected_kind: RelocKind| {
            let mut codegen = test_codegen_with_rom_section();
            let operand = Operand::Immediate {
                value: Expr::MacroCall {
                    name: macro_name.to_string(),
                    arg: Box::new(Expr::Ident {
                        name: "sym".to_string(),
                    }),
                },
            };
            codegen.compile_asm("lda", &[operand]);
            assert_eq!(codegen.sections[0].data, vec![0xA9, 0x00]);
            let relocs = &codegen.sections[0].relocations;
            assert_eq!(relocs.len(), 1);
            assert_eq!(relocs[0].symbol, "sym");
            assert_eq!(relocs[0].kind, expected_kind);
            assert_eq!(relocs[0].addend, 0);
        };

        mk("lo", RelocKind::Lo8);
        mk("hi", RelocKind::Hi8);
    }

    /// An immediate operand that is neither a constant nor a symbol
    /// produces error 305 instead of a silent zero byte.
    #[test]
    fn unresolvable_immediate_emits_error_305() {
        let mut codegen = test_codegen_with_rom_section();
        let operand = Operand::Immediate {
            value: Expr::FnCall {
                name: "unknown".to_string(),
                args: vec![],
            },
        };
        codegen.compile_asm("lda", &[operand]);
        assert_eq!(codegen.sections[0].data, vec![0xA9, 0x00]);
        assert!(codegen
            .diags
            .iter()
            .any(|d| { d.severity == op_diagnostics::Severity::Error && d.code == 305 }));
    }

    /// `len!(HELLO)` resolves to the element count of the array type.
    #[test]
    fn len_macro_resolves_array_element_count() {
        let (codegen, _) = walk_parsed_source(
            "#[interrupt(reset)]\nfn main() {\n    lda #len!(HELLO)\n    rts\n}\n\
             const HELLO: [u8; 11] = \"Hello, NES!\";\n",
        );
        // main is a root via #[interrupt(reset)], so it is placed first:
        // LDA #11, RTS. The HELLO const is referenced by len!(HELLO), so
        // it is placed after main in the same section.
        assert_eq!(&codegen.sections[0].data[..3], &[0xA9, 0x0B, 0x60]);
        // The const data follows: "Hello, NES!" (11 bytes).
        assert_eq!(&codegen.sections[0].data[3..14], b"Hello, NES!");
    }

    /// `sizeof!(ptr)` resolves to the byte size of the pointer type.
    #[test]
    fn sizeof_macro_resolves_type_size() {
        let (codegen, _) = walk_parsed_source(
            "#[interrupt(reset)]\nfn main() {\n    lda #sizeof!(ptr)\n    rts\n}\n\
             ptr: pointer;\n",
        );
        assert_eq!(&codegen.sections[0].data[..3], &[0xA9, 0x02, 0x60]);
        assert!(codegen.sections[0].relocations.is_empty());
    }

    /// `len!` on a non-array type is not a constant; the codegen emits
    /// error 305.
    #[test]
    fn len_macro_on_non_array_emits_error() {
        let (codegen, _) = walk_parsed_source(
            "#[interrupt(reset)]\nfn main() {\n    lda #len!(scalar)\n    rts\n}\n\
             scalar: u8;\n",
        );
        assert_eq!(&codegen.sections[0].data[..3], &[0xA9, 0x00, 0x60]);
        assert!(codegen
            .diags
            .iter()
            .any(|d| d.severity == op_diagnostics::Severity::Error && d.code == 305));
    }

    /// Expand an inline fn with two parameters and check the emitted
    /// bytes against a hand-assembled equivalent.
    #[test]
    fn inline_fn_substitutes_two_params() {
        let mut codegen = test_codegen_with_rom_section();
        // inline fn copy(src, dst) { lda src; sta dst }
        codegen.inline_fns.insert(
            "copy".to_string(),
            InlineFn {
                params: vec!["src".to_string(), "dst".to_string()],
                body: vec![
                    FnStmt::AsmStmt {
                        opcode: "lda".to_string(),
                        operands: vec![Operand::MemoryOperand {
                            mode_prefix: None,
                            expr: Expr::Ident {
                                name: "src".to_string(),
                            },
                            index_reg: None,
                            is_indirect: false,
                        }],
                    },
                    FnStmt::AsmStmt {
                        opcode: "sta".to_string(),
                        operands: vec![Operand::MemoryOperand {
                            mode_prefix: None,
                            expr: Expr::Ident {
                                name: "dst".to_string(),
                            },
                            index_reg: None,
                            is_indirect: false,
                        }],
                    },
                ],
                is_inline: true,
            },
        );

        codegen.compile_fn_call(
            "copy",
            &[
                Expr::Number { value: 0x2000 },
                Expr::Number { value: 0x4000 },
            ],
        );

        // Hand-assembled equivalent: lda $2000 / sta $4000.
        assert_eq!(
            codegen.sections[0].data,
            vec![0xAD, 0x00, 0x20, 0x8D, 0x00, 0x40]
        );
        assert!(codegen.sections[0].relocations.is_empty());
    }

    /// A nested inline call in the substituted body resolves during
    /// the compile pass.
    #[test]
    fn inline_fn_expands_nested_calls() {
        let mut codegen = test_codegen_with_rom_section();
        // inline fn pause() { nop }
        codegen.inline_fns.insert(
            "pause".to_string(),
            InlineFn {
                params: Vec::new(),
                body: vec![FnStmt::AsmStmt {
                    opcode: "nop".to_string(),
                    operands: Vec::new(),
                }],
                is_inline: true,
            },
        );
        // inline fn step(value) { pause(); lda value }
        codegen.inline_fns.insert(
            "step".to_string(),
            InlineFn {
                params: vec!["value".to_string()],
                body: vec![
                    FnStmt::FnCall {
                        name: "pause".to_string(),
                        args: Vec::new(),
                    },
                    FnStmt::AsmStmt {
                        opcode: "lda".to_string(),
                        operands: vec![Operand::MemoryOperand {
                            mode_prefix: None,
                            expr: Expr::Ident {
                                name: "value".to_string(),
                            },
                            index_reg: None,
                            is_indirect: false,
                        }],
                    },
                ],
                is_inline: true,
            },
        );

        codegen.compile_fn_call("step", &[Expr::Number { value: 0x42 }]);

        // Hand-assembled equivalent: nop / lda $42.
        assert_eq!(codegen.sections[0].data, vec![0xEA, 0xA5, 0x42]);
        assert!(codegen.sections[0].relocations.is_empty());
    }

    /// Substitution reaches into nested call arguments.
    #[test]
    fn inline_fn_substitutes_nested_call_args() {
        let mut codegen = test_codegen_with_rom_section();
        // inline fn write(value) { lda value }
        codegen.inline_fns.insert(
            "write".to_string(),
            InlineFn {
                params: vec!["value".to_string()],
                body: vec![FnStmt::AsmStmt {
                    opcode: "lda".to_string(),
                    operands: vec![Operand::MemoryOperand {
                        mode_prefix: None,
                        expr: Expr::Ident {
                            name: "value".to_string(),
                        },
                        index_reg: None,
                        is_indirect: false,
                    }],
                }],
                is_inline: true,
            },
        );

        // inline fn pipe(value) { write(value) }
        codegen.inline_fns.insert(
            "pipe".to_string(),
            InlineFn {
                params: vec!["value".to_string()],
                body: vec![FnStmt::FnCall {
                    name: "write".to_string(),
                    args: vec![Expr::Ident {
                        name: "value".to_string(),
                    }],
                }],
                is_inline: true,
            },
        );

        codegen.compile_fn_call("pipe", &[Expr::Number { value: 0x42 }]);

        // pipe(0x42) -> write(0x42) -> lda $42.
        assert_eq!(codegen.sections[0].data, vec![0xA5, 0x42]);
        assert!(codegen.sections[0].relocations.is_empty());
    }

    /// Parse `source` and run the two-pass module walk on it.
    fn walk_parsed_source(source: &str) -> (Codegen, Vec<op_diagnostics::Diagnostic>) {
        let (ast, diags) =
            crate::parser::parse_source("multi-pass.op", source, "rp2A03-nintendo-nes-ntsc", &[]);
        assert!(diags.is_empty(), "parse diagnostics: {diags:?}");
        let mut codegen = test_codegen_with_rom_section();
        codegen.walk_module(&ast.root);
        (codegen, diags)
    }

    #[test]
    fn non_inline_fn_call_emits_jsr_relocation() {
        // A non-inline `fn` is placed once in the section; calls to it
        // emit `jsr` plus an `Abs16` relocation rather than inlining.
        let (codegen, diags) = walk_parsed_source(
            "fn helper() {\n    lda #5\n    rts\n}\nfn caller() {\n    helper()\n    rts\n}\n",
        );
        let errors: Vec<_> = diags
            .iter()
            .filter(|d| d.severity == op_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");

        let rom = &codegen.sections[0];
        // helper compiles to lda #5 (A9 05) + rts (60); it must appear
        // exactly once in the section.
        let helper_bytes: [u8; 3] = [0xA9, 0x05, 0x60];
        let helper_count = rom.data.windows(3).filter(|w| *w == helper_bytes).count();
        assert_eq!(helper_count, 1, "helper body should appear exactly once");
        // caller compiles to jsr helper (20 00 00) + rts (60).
        assert!(rom.data.contains(&0x20), "caller should emit a jsr opcode");
        // Exactly one Abs16 relocation against helper.
        let helper_relocs: Vec<_> = rom
            .relocations
            .iter()
            .filter(|r| r.symbol == "helper")
            .collect();
        assert_eq!(
            helper_relocs.len(),
            1,
            "one relocation against helper, got {helper_relocs:?}"
        );
        assert_eq!(helper_relocs[0].kind, RelocKind::Abs16);
    }

    #[test]
    fn multi_pass_resolves_const_declared_after_fn() {
        let (codegen, _) = walk_parsed_source(
            "fn early() {\n    lda ANSWER\n    rts\n}\nconst ANSWER: u8 = 42;\n",
        );
        // lda $42 (zero-page) / rts, no relocation for ANSWER.
        assert_eq!(codegen.sections[0].data, vec![0xA5, 0x2A, 0x60]);
        assert!(codegen.sections[0].relocations.is_empty());
    }

    #[test]
    fn multi_pass_resolves_use_declared_after_fn() {
        let tmp = std::env::temp_dir().join(format!("opc-multi-pass-use-{}", std::process::id()));
        let std_root = tmp.join("std/src");
        write_fake_std(&std_root);

        with_std_env(&std_root, || {
            let (codegen, _) =
                walk_parsed_source("fn early() {\n    lda CYCLES\n    rts\n}\nuse std::cpu::*;\n");
            // CYCLES is 1; lda $1 (zero-page) / rts, no relocation.
            assert_eq!(codegen.sections[0].data, vec![0xA5, 0x01, 0x60]);
            assert!(codegen.sections[0].relocations.is_empty());
            assert!(codegen.diags.is_empty());
        });
    }

    #[test]
    fn multi_pass_const_references_earlier_const() {
        let (codegen, _) = walk_parsed_source(
            "fn early() {\n    lda B\n    rts\n}\nconst A: u8 = 40;\nconst B: u8 = A + 2;\n",
        );
        // B evaluates to 42 against the collected value of A.
        assert_eq!(codegen.sections[0].data, vec![0xA5, 0x2A, 0x60]);
        assert!(codegen.sections[0].relocations.is_empty());
    }

    #[test]
    fn use_tree_resolves_super_in_std_modules() {
        let tmp = std::env::temp_dir().join(format!("opc-super-use-{}", std::process::id()));
        let std_root = tmp.join("std/src");
        write_nes_std(&std_root);

        with_std_env(&std_root, || {
            // The user imports the macros module directly, so the
            // constants names can only arrive through macros.op's
            // private `use super::constants::*;`.
            let (codegen, _) = walk_parsed_source(
                "use std::nes::macros::*;\n#[interrupt(reset)]\nfn main() {\n    vblank_on()\n    rts\n}\n",
            );
            // vblank_on expands to `sta ST_VBLANK` -> sta $80
            // (zero-page) / rts, with no relocation and no
            // diagnostics.
            assert_eq!(codegen.sections[0].data, vec![0x85, 0x80, 0x60]);
            assert!(codegen.sections[0].relocations.is_empty());
            assert!(codegen.diags.is_empty());
        });
    }

    #[test]
    fn module_cache_records_source_directory() {
        let tmp = std::env::temp_dir().join(format!("opc-module-dir-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let tmp = tmp.canonicalize().unwrap();
        let file = tmp.join("dirtest.op");
        std::fs::write(&file, "const X: u8 = 1;\n").unwrap();
        let target = TargetTriplet::parse("rp2A03-nintendo-nes-ntsc").unwrap();

        let mut cache = ModuleCache::default();
        cache.load_module(&file, &target, &[]).unwrap();

        assert_eq!(cache.dir_of(&file), Some(&tmp));
        assert_eq!(cache.dir_of(&tmp.join("missing.op")), None);
    }

    #[test]
    fn locate_attr_file_resolves_relative_to_source_dir() {
        let tmp = std::env::temp_dir().join(format!("opc-locate-src-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("blob.bin"), [0x11u8, 0x22, 0x33]).unwrap();

        let mut codegen = test_codegen_with_rom_section();
        codegen.source_dir = tmp.clone();
        codegen.current_section = Some(0);
        codegen.emit_file_bytes("blob.bin", "test");
        codegen.current_section = None;

        assert_eq!(codegen.sections[0].data, vec![0x11, 0x22, 0x33]);
        assert!(codegen.diags.is_empty());
    }

    #[test]
    fn locate_addr_pads_gap_and_pins_symbol() {
        use op_common::ast::{AttrArg, Attribute};

        let mut codegen = test_codegen_with_rom_section();
        codegen.current_section = Some(0);
        codegen.pad_byte = 0xFF;
        assert!(codegen.locate_to_addr(0x8010, "tab")); // org 0x8000 + 0x10
        codegen.sections[0].data.extend_from_slice(&[0xAA, 0xBB]);
        codegen.sections[0].symbols.push(Symbol {
            name: "tab".to_string(),
            offset: 0x10,
            size: 2,
            kind: SymbolKind::Variable,
            is_pub: false,
        });
        codegen.current_section = None;

        assert_eq!(codegen.sections[0].data[0], 0xFF);
        assert_eq!(codegen.sections[0].data[0x10], 0xAA);
        assert_eq!(codegen.sections[0].data[0x11], 0xBB);
        assert_eq!(
            codegen.sections[0]
                .symbols
                .iter()
                .find(|s| s.name == "tab")
                .unwrap()
                .offset,
            0x10
        );
        assert!(codegen.diags.is_empty());

        // Overlap detection.
        codegen.current_section = Some(0);
        codegen.locate_to_addr(0x8010, "again");
        assert!(codegen.diags.iter().any(|d| d.severity == Severity::Error));
        codegen.current_section = None;

        // Attribute parsing.
        let attr = Attribute {
            path: "locate".to_string(),
            args: vec![
                AttrArg {
                    name: "addr".to_string(),
                    value: "0x0080".to_string(),
                    sub_args: Vec::new(),
                },
                AttrArg {
                    name: "file".to_string(),
                    value: "\"blob.bin\"".to_string(),
                    sub_args: Vec::new(),
                },
            ],
        };
        let (addr, file) = Codegen::find_locate_attr(&[attr]).unwrap();
        assert_eq!(addr, Some(0x80));
        assert_eq!(file.as_deref(), Some("blob.bin"));
    }

    #[test]
    fn locate_str_in_std_module_resolves_relative_to_std_file() {
        let tmp = std::env::temp_dir().join(format!("opc-locate-std-{}", std::process::id()));
        let std_root = tmp.join("std/src");
        std::fs::create_dir_all(&std_root).unwrap();
        // The std module declares a pinned const. The pin at 0x8010 sits
        // 0x10 bytes into the org-0x8000 test ROM section. Import alone
        // (no reference) places it: the pin is a placement directive.
        std::fs::write(std_root.join("lib.op"), "pub mod res;\n").unwrap();
        std::fs::write(
            std_root.join("res.op"),
            "#[locate(addr = 0x8010)]\nconst DATA: [u8; 4] = [0xDE, 0xAD, 0xBE, 0xEF];\n",
        )
        .unwrap();

        with_std_env(&std_root, || {
            let mut codegen = test_codegen_with_rom_section();
            codegen.source_dir = std_root.clone();
            let tree = UseTree::Path {
                root: UseRoot::Name("std".into()),
                segments: vec!["res".to_string()],
                tail: UseTail::Glob,
            };
            codegen.resolve_use_decl(&[tree]);
            // The pin on the imported const drives placement without fn
            // roots or references.
            let items: Vec<op_common::ast::Item> = Vec::new();
            codegen.placement_pass(&items);

            assert_eq!(codegen.sections[0].data.len(), 0x14);
            assert_eq!(&codegen.sections[0].data[0x10..], &[0xDE, 0xAD, 0xBE, 0xEF]);
        });
    }
}
