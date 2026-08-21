//! Codegen and optimizer unit tests.
//!
//! These tests call `opc::codegen::compile_source()` on parsed ASTs and
//! assert the `ObjectFile` structure (sections, symbols, relocations,
//! data bytes).

use op_ir::{ObjectFile, SectionKind, SymbolKind};
use opc::codegen::{compile_source, compile_source_with_tables, NameTables};
use opc::parser::parse_source;

/// Helper: parse and compile a source string with the 6502 target.
fn compile(src: &str) -> ObjectFile {
    let (ast, _diags) = parse_source("test.op", src, "rp2A03-nintendo-nes-ntsc", &[]);
    let (obj, _codegen_diags) = compile_source(&ast, 1, &[], &[]);
    obj
}

/// Helper: parse and compile with a specific opt level.
fn compile_with_opt(src: &str, opt_level: u8) -> ObjectFile {
    let (ast, _diags) = parse_source("test.op", src, "rp2A03-nintendo-nes-ntsc", &[]);
    let (obj, _) = compile_source(&ast, opt_level, &[], &[]);
    obj
}

// === Section creation ======================================================

#[test]
fn codegen_rom_section() {
    let obj = compile("#[rom(org = 0xC000, bank = 0, maxsize = 0x4000)] { fn main() { } }");
    assert_eq!(obj.sections.len(), 1);
    let s = &obj.sections[0];
    assert_eq!(s.kind, SectionKind::Rom);
    assert_eq!(s.org, 0xC000);
    assert_eq!(s.bank, 0);
    assert_eq!(s.maxsize, 0x4000);
}

#[test]
fn codegen_ram_section() {
    let obj = compile("#[ram(org = 0x0000, maxsize = 0x100)] { counter: u8; }");
    assert_eq!(obj.sections.len(), 1);
    let s = &obj.sections[0];
    assert_eq!(s.kind, SectionKind::Ram);
    assert_eq!(s.org, 0x0000);
    assert_eq!(s.maxsize, 0x100);
}

#[test]
fn codegen_chr_section() {
    let obj = compile("#[chr(bank = 0)] { }");
    assert_eq!(obj.sections.len(), 1);
    let s = &obj.sections[0];
    assert_eq!(s.kind, SectionKind::Chr);
    assert_eq!(s.bank, 0);
}

#[test]
fn codegen_multiple_sections() {
    let obj = compile(
        "#[rom(org = 0xC000, bank = 0, maxsize = 0x4000)] { fn main() { } }
         #[ram(org = 0x0000, maxsize = 0x100)] { counter: u8; }",
    );
    assert_eq!(obj.sections.len(), 2);
    assert_eq!(obj.sections[0].kind, SectionKind::Rom);
    assert_eq!(obj.sections[1].kind, SectionKind::Ram);
}

// === Symbol recording =====================================================

#[test]
fn codegen_function_symbol() {
    let obj = compile("#[rom(org = 0xC000, bank = 0, maxsize = 0x4000)] { fn main() { lda 0 } }");
    let s = &obj.sections[0];
    assert!(s
        .symbols
        .iter()
        .any(|sym| { sym.name == "main" && sym.kind == SymbolKind::Function && sym.offset == 0 }));
}

#[test]
fn codegen_variable_symbol() {
    let obj = compile("#[ram(org = 0x0000, maxsize = 0x100)] { counter: u8; }");
    let s = &obj.sections[0];
    assert!(s
        .symbols
        .iter()
        .any(|sym| { sym.name == "counter" && sym.kind == SymbolKind::Variable && sym.size == 1 }));
}

#[test]
fn codegen_label_symbol() {
    let obj =
        compile("#[rom(org = 0xC000, bank = 0, maxsize = 0x4000)] { fn main() { 'loop: inx } }");
    let s = &obj.sections[0];
    assert!(s
        .symbols
        .iter()
        .any(|sym| { sym.name == "loop" && sym.kind == SymbolKind::Label }));
}

// === Opcode encoding ======================================================

#[test]
fn codegen_lda_immediate() {
    let obj = compile("#[rom(org = 0, bank = 0, maxsize = 0x100)] { fn f() { lda #0 } }");
    let data = &obj.sections[0].data;
    // LDA immediate: A9 00
    assert_eq!(data[0], 0xA9);
    assert_eq!(data[1], 0x00);
}

#[test]
fn codegen_lda_absolute() {
    let obj = compile("#[rom(org = 0, bank = 0, maxsize = 0x100)] { fn f() { lda 0x2000 } }");
    let data = &obj.sections[0].data;
    // LDA absolute: AD 00 20
    assert_eq!(data[0], 0xAD);
    assert_eq!(data[1], 0x00);
    assert_eq!(data[2], 0x20);
}

#[test]
fn codegen_lda_zeropage() {
    let obj = compile("#[rom(org = 0, bank = 0, maxsize = 0x100)] { fn f() { lda 0x20 } }");
    let data = &obj.sections[0].data;
    // LDA zero-page: A5 20
    assert_eq!(data[0], 0xA5);
    assert_eq!(data[1], 0x20);
}

#[test]
fn codegen_sta_absolute() {
    let obj = compile("#[rom(org = 0, bank = 0, maxsize = 0x100)] { fn f() { sta 0x2000 } }");
    let data = &obj.sections[0].data;
    // STA absolute: 8D 00 20
    assert_eq!(data[0], 0x8D);
    assert_eq!(data[1], 0x00);
    assert_eq!(data[2], 0x20);
}

#[test]
fn codegen_inx_implied() {
    let obj = compile("#[rom(org = 0, bank = 0, maxsize = 0x100)] { fn f() { inx } }");
    let data = &obj.sections[0].data;
    // INX implied: E8
    assert_eq!(data[0], 0xE8);
    assert_eq!(data.len(), 2); // INX + implicit RTS
}

#[test]
fn codegen_clc_implied() {
    let obj = compile("#[rom(org = 0, bank = 0, maxsize = 0x100)] { fn f() { clc } }");
    let data = &obj.sections[0].data;
    // CLC implied: 18
    assert_eq!(data[0], 0x18);
}

#[test]
fn codegen_rts_return() {
    let obj = compile("#[rom(org = 0, bank = 0, maxsize = 0x100)] { fn f() { return } }");
    let data = &obj.sections[0].data;
    // RTS: 60
    assert_eq!(data[0], 0x60);
}

#[test]
fn codegen_jsr_call() {
    let obj = compile(
        "#[rom(org = 0, bank = 0, maxsize = 0x100)] {
            fn main() { foo() }
            fn foo() { inx }
        }",
    );
    let data = &obj.sections[0].data;
    // JSR: 20 xx xx
    assert_eq!(data[0], 0x20);
    // The operand bytes are placeholders (relocation).
    assert_eq!(data[1], 0x00);
    assert_eq!(data[2], 0x00);
}

// === Addressing modes =====================================================

#[test]
fn codegen_immediate_mode() {
    let obj = compile("#[rom(org = 0, bank = 0, maxsize = 0x100)] { fn f() { lda #0xFF } }");
    let data = &obj.sections[0].data;
    assert_eq!(data[0], 0xA9); // LDA immediate
    assert_eq!(data[1], 0xFF);
}

#[test]
fn codegen_zeropage_mode() {
    let obj = compile("#[rom(org = 0, bank = 0, maxsize = 0x100)] { fn f() { lda 0x42 } }");
    let data = &obj.sections[0].data;
    assert_eq!(data[0], 0xA5); // LDA zero-page
    assert_eq!(data[1], 0x42);
    assert_eq!(data.len(), 3); // 2-byte instr + implicit RTS
}

#[test]
fn codegen_absolute_mode() {
    let obj = compile("#[rom(org = 0, bank = 0, maxsize = 0x100)] { fn f() { lda 0x2000 } }");
    let data = &obj.sections[0].data;
    assert_eq!(data[0], 0xAD); // LDA absolute
    assert_eq!(data.len(), 4); // 3-byte instr + implicit RTS
}

#[test]
fn codegen_forced_zp_mode() {
    let obj = compile("#[rom(org = 0, bank = 0, maxsize = 0x100)] { fn f() { lda zp 0x2000 } }");
    let data = &obj.sections[0].data;
    // Forced zero-page should use zero-page encoding even for larger addresses.
    assert_eq!(data[0], 0xA5); // LDA zero-page
    assert_eq!(data[1], 0x00); // truncated to 1 byte
    assert_eq!(data.len(), 3); // 2-byte instr + implicit RTS
}

// === Relocations ==========================================================

#[test]
fn codegen_relocation_for_jsr() {
    let obj = compile(
        "#[rom(org = 0, bank = 0, maxsize = 0x100)] {
            fn main() { foo() }
            fn foo() { inx }
        }",
    );
    let s = &obj.sections[0];
    assert!(s.relocations.iter().any(|r| r.symbol == "foo"));
}

#[test]
fn codegen_relocation_for_jmp_label() {
    let obj = compile(
        "#[rom(org = 0, bank = 0, maxsize = 0x100)] {
            fn f() { jmp 'loop }
        }",
    );
    let s = &obj.sections[0];
    // jmp 'loop should produce a relocation for the label.
    assert!(s.relocations.iter().any(|r| r.symbol == "loop"));
}

// === Control flow =========================================================

#[test]
fn codegen_if_statement() {
    let obj = compile(
        "#[rom(org = 0, bank = 0, maxsize = 0x100)] {
            fn f() { if (set) { lda 0 } }
        }",
    );
    let data = &obj.sections[0].data;
    // BEQ (branch if zero, i.e. not-set) + offset + LDA
    assert_eq!(data[0], 0xF0); // BEQ
    assert!(data.len() > 2);
}

#[test]
fn codegen_if_else_statement() {
    let obj = compile(
        "#[rom(org = 0, bank = 0, maxsize = 0x100)] {
            fn f() { if (set) { lda 0 } else { lda 1 } }
        }",
    );
    let data = &obj.sections[0].data;
    // BEQ + offset + LDA #0 + JMP + LDA #1
    assert_eq!(data[0], 0xF0); // BEQ
    assert!(data.len() > 5);
}

#[test]
fn codegen_while_statement() {
    let obj = compile(
        "#[rom(org = 0, bank = 0, maxsize = 0x100)] {
            fn f() { while (not zero) { dex } }
        }",
    );
    let data = &obj.sections[0].data;
    // BEQ (branch if zero, i.e. not-condition for "not zero") + offset + DEX + JMP
    assert!(data.len() > 3);
}

#[test]
fn codegen_do_while_statement() {
    let obj = compile(
        "#[rom(org = 0, bank = 0, maxsize = 0x100)] {
            fn f() { do { inx } while (set) }
        }",
    );
    let data = &obj.sections[0].data;
    // INX + BNE (branch if set) back
    assert_eq!(data[0], 0xE8); // INX
    assert!(data.len() > 2);
}

#[test]
fn codegen_loop_statement() {
    let obj = compile(
        "#[rom(org = 0, bank = 0, maxsize = 0x100)] {
            fn f() { loop { inx } }
        }",
    );
    let data = &obj.sections[0].data;
    // INX + JMP back
    assert_eq!(data[0], 0xE8); // INX
    assert_eq!(data[data.len() - 3], 0x4C); // JMP
}

#[test]
fn codegen_return_statement() {
    let obj = compile(
        "#[rom(org = 0, bank = 0, maxsize = 0x100)] {
            fn f() { return }
        }",
    );
    let data = &obj.sections[0].data;
    assert_eq!(data[0], 0x60); // RTS
}

// === Inline fn expansion ==================================================

#[test]
fn codegen_inline_fn_expansion() {
    let obj = compile(
        "#[rom(org = 0, bank = 0, maxsize = 0x100)] {
            inline fn do_inc() { inx }
            fn main() { do_inc() }
        }",
    );
    let data = &obj.sections[0].data;
    // The inline fn body (inx = E8) should be expanded at the call site.
    assert_eq!(data[0], 0xE8); // INX
}

// === Variable allocation ==================================================

#[test]
fn codegen_variable_allocation() {
    let obj = compile(
        "#[ram(org = 0x0000, maxsize = 0x100)] {
            counter: u8;
            flag: u8;
        }",
    );
    let s = &obj.sections[0];
    assert_eq!(s.data.len(), 2); // 2 bytes for 2 u8 variables
    assert!(s
        .symbols
        .iter()
        .any(|sym| sym.name == "counter" && sym.offset == 0));
    assert!(s
        .symbols
        .iter()
        .any(|sym| sym.name == "flag" && sym.offset == 1));
}

#[test]
fn codegen_u16_variable() {
    let obj = compile("#[ram(org = 0, maxsize = 0x100)] { ptr: u16; }");
    let s = &obj.sections[0];
    assert_eq!(s.data.len(), 2); // u16 = 2 bytes
    assert!(s
        .symbols
        .iter()
        .any(|sym| sym.name == "ptr" && sym.size == 2));
}

// === Optimizer transforms =================================================

#[test]
fn optimizer_redundant_load() {
    let src = "#[rom(org = 0, bank = 0, maxsize = 0x100)] { fn f() { lda #0 lda #0 inx } }";
    let opt = compile_with_opt(src, 1);
    let nopt = compile_with_opt(src, 0);
    // Optimized should be shorter (one lda removed).
    assert!(opt.sections[0].data.len() < nopt.sections[0].data.len());
}

#[test]
fn optimizer_redundant_store() {
    let src = "#[rom(org = 0, bank = 0, maxsize = 0x100)] { fn f() { sta 0x0100 sta 0x0100 inx } }";
    let opt = compile_with_opt(src, 1);
    let nopt = compile_with_opt(src, 0);
    assert!(opt.sections[0].data.len() < nopt.sections[0].data.len());
}

#[test]
fn optimizer_stack_push_pop() {
    let src = "#[rom(org = 0, bank = 0, maxsize = 0x100)] { fn f() { pha pla inx } }";
    let opt = compile_with_opt(src, 1);
    let nopt = compile_with_opt(src, 0);
    // pha + pla should be removed.
    assert!(opt.sections[0].data.len() < nopt.sections[0].data.len());
}

#[test]
fn optimizer_strength_reduce() {
    let src = "#[rom(org = 0, bank = 0, maxsize = 0x100)] { fn f() { lda #0 clc adc #0 inx } }";
    let opt = compile_with_opt(src, 1);
    let nopt = compile_with_opt(src, 0);
    // clc + adc #0 should be removed.
    assert!(opt.sections[0].data.len() < nopt.sections[0].data.len());
}

#[test]
fn optimizer_disabled() {
    let src = "#[rom(org = 0, bank = 0, maxsize = 0x100)] { fn f() { lda #0 lda #0 inx } }";
    let opt0 = compile_with_opt(src, 0);
    // With opt_level=0, both lda instructions should be present.
    let data = &opt0.sections[0].data;
    assert_eq!(data[0], 0xA9); // first lda
    assert_eq!(data[2], 0xA9); // second lda (not removed)
}

#[test]
fn optimizer_skips_chr_sections() {
    // CHR sections hold pattern-table data, not code. The optimizer must
    // not decode the bytes as 6502 instructions or apply transforms.
    // The byte sequence below contains patterns that the optimizer would
    // drop if it ran on the section (pha/pla = 0x48/0x68, redundant loads).
    use op_ir::{Section, SectionKind};
    use opc::optimizer::optimize;
    let chr_bytes: Vec<u8> = vec![
        0xA9, 0x00, 0xA9, 0x00, // redundant lda #0, lda #0
        0x48, 0x68, // pha, pla (push-pop pair)
        0x00, 0x18, 0x00, 0x18, // brk/clc pattern bytes
        0xEA, 0xEA, 0xEA, 0xEA, // nop nop nop nop
    ];
    let mut sections = vec![Section {
        name: "chr_bank0".to_string(),
        kind: SectionKind::Chr,
        org: 0,
        bank: 0,
        maxsize: 0,
        symbols: Vec::new(),
        relocations: Vec::new(),
        data: chr_bytes.clone(),
    }];
    optimize(&mut sections, 1);
    // The CHR data must equal the input bytes exactly. The optimizer must
    // not remove or change any byte.
    assert_eq!(sections[0].data, chr_bytes, "optimizer corrupted CHR data");
    assert_eq!(
        sections[0].data.len(),
        chr_bytes.len(),
        "optimizer changed CHR section length"
    );
}

#[test]
fn optimizer_skips_ram_sections() {
    // RAM sections hold variable data and initialization bytes, not code.
    // The optimizer must not run on them.
    use op_ir::{Section, SectionKind};
    use opc::optimizer::optimize;
    let ram_bytes: Vec<u8> = vec![0xA9, 0x00, 0xA9, 0x00, 0x48, 0x68, 0xEA, 0xEA];
    let mut sections = vec![Section {
        name: "ram_bank0".to_string(),
        kind: SectionKind::Ram,
        org: 0x0000,
        bank: 0,
        maxsize: 0x100,
        symbols: Vec::new(),
        relocations: Vec::new(),
        data: ram_bytes.clone(),
    }];
    optimize(&mut sections, 1);
    assert_eq!(sections[0].data, ram_bytes, "optimizer corrupted RAM data");
}

#[test]
fn optimizer_relocation_remaps_to_instruction_boundary() {
    // When a peephole transform drops an instruction, the relocations that
    // follow must map to the correct new offset. This test places a
    // relocation (Abs16 against a symbol) inside a 3-byte instruction,
    // with a redundant load before it that the optimizer will remove.
    // The relocation offset must shift by the size of the dropped
    // instruction (2 bytes for lda #0).
    use op_ir::RelocKind;
    let src = "#[rom(org = 0, bank = 0, maxsize = 0x100)] {
        fn f() {
            lda #0
            lda #0
            jsr target
        }
    }";
    let opt = compile_with_opt(src, 1);
    let nopt = compile_with_opt(src, 0);
    let rom = opt
        .sections
        .iter()
        .find(|s| s.kind == SectionKind::Rom)
        .expect("ROM section must exist");
    let rom0 = nopt
        .sections
        .iter()
        .find(|s| s.kind == SectionKind::Rom)
        .expect("ROM section must exist");
    // The optimized ROM must be shorter: one lda #0 (2 bytes) removed.
    assert!(
        rom.data.len() < rom0.data.len(),
        "optimizer did not remove the redundant load"
    );
    // The jsr relocation must still be present and point at the jsr
    // instruction's operand byte (offset = jsr_pos + 1).
    let jsr_reloc = rom
        .relocations
        .iter()
        .find(|r| r.kind == RelocKind::Abs16 && r.symbol == "target")
        .expect("jsr target relocation must survive optimization");
    // The jsr opcode must be at offset = jsr_reloc.offset - 1.
    let jsr_offset = jsr_reloc.offset as usize;
    assert!(jsr_offset >= 1, "relocation offset too small");
    assert_eq!(
        rom.data[jsr_offset - 1],
        0x20,
        "byte before relocation must be the jsr opcode 0x20"
    );
}

#[test]
fn optimizer_changes_rom_but_not_chr() {
    // Build an object with one ROM section and one CHR section. Run the
    // optimizer at level 1. The ROM data must change (the redundant load is
    // folded) and the CHR data must stay byte-for-byte identical.
    use op_ir::{Section, SectionKind};
    use opc::optimizer::optimize;

    // ROM bytes: lda #0, lda #0 (redundant pair the optimizer folds to one).
    let rom_bytes: Vec<u8> = vec![0xA9, 0x00, 0xA9, 0x00];
    // CHR bytes: a pattern that would be corrupted if the optimizer ran on it.
    let chr_bytes: Vec<u8> = vec![0xA9, 0x00, 0xA9, 0x00, 0x48, 0x68, 0xEA, 0xEA];

    let mut sections = vec![
        Section {
            name: "rom_bank0".to_string(),
            kind: SectionKind::Rom,
            org: 0xC000,
            bank: 0,
            maxsize: 0x4000,
            symbols: Vec::new(),
            relocations: Vec::new(),
            data: rom_bytes.clone(),
        },
        Section {
            name: "chr_bank0".to_string(),
            kind: SectionKind::Chr,
            org: 0,
            bank: 0,
            maxsize: 0,
            symbols: Vec::new(),
            relocations: Vec::new(),
            data: chr_bytes.clone(),
        },
    ];

    optimize(&mut sections, 1);

    // The ROM section must change: the redundant lda #0 pair collapses to
    // a single lda #0 (2 bytes instead of 4).
    assert_eq!(
        sections[0].data.len(),
        2,
        "optimizer must fold the redundant lda pair in ROM"
    );
    assert_ne!(
        sections[0].data, rom_bytes,
        "ROM data must change after optimization"
    );

    // The CHR section must not change at all.
    assert_eq!(
        sections[1].data, chr_bytes,
        "optimizer must not change CHR data"
    );
    assert_eq!(
        sections[1].data.len(),
        chr_bytes.len(),
        "CHR section length must not change"
    );
}

// === Other CPU families ===================================================

#[test]
fn codegen_z80_target() {
    let (ast, _) = parse_source("test.op", "fn f() { nop }", "z80-nintendo-gameboy", &[]);
    let (obj, _) = compile_source(&ast, 1, &[], &[]);
    // Z80 should produce at least an empty or minimal output.
    assert_eq!(obj.target, "z80-nintendo-gameboy");
}

#[test]
fn codegen_68000_target() {
    let (ast, _) = parse_source("test.op", "fn f() { nop }", "m68000-sega-genesis", &[]);
    let (obj, _) = compile_source(&ast, 1, &[], &[]);
    assert_eq!(obj.target, "m68000-sega-genesis");
}

#[test]
fn codegen_65c816_target() {
    let (ast, _) = parse_source("test.op", "fn f() { nop }", "wdc65c816-nintendo-snes", &[]);
    let (obj, _) = compile_source(&ast, 1, &[], &[]);
    assert_eq!(obj.target, "wdc65c816-nintendo-snes");
}

// === Empty source =========================================================

#[test]
fn codegen_empty_source() {
    let (ast, _) = parse_source("test.op", "", "rp2A03-nintendo-nes-ntsc", &[]);
    let (obj, _) = compile_source(&ast, 1, &[], &[]);
    assert_eq!(obj.sections.len(), 0);
}

#[test]
fn codegen_no_block_attributes() {
    // Functions without a #[rom] block produce no sections.
    let obj = compile("fn f() { lda 0 }");
    assert_eq!(obj.sections.len(), 0);
}

// === Std library resolution ================================================
//
// These tests resolve the real std library (the directory named by
// `OP_STD_PATH`, or the std repository that is a sibling of this
// workspace) and verify what `use std::...` declarations import. They
// skip themselves when std is not available.

/// Locate the std crate root (the directory that contains `lib.op`).
///
/// Checks `OP_STD_PATH` first, then the std repository that is a
/// sibling of this workspace. Returns `None` when std is not
/// available.
fn std_root() -> Option<std::path::PathBuf> {
    if let Some(root) = std::env::var_os("OP_STD_PATH") {
        let root = std::path::PathBuf::from(root);
        if root.join("lib.op").is_file() {
            return Some(root);
        }
    }
    let sibling = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("std")
        .join("src");
    if sibling.join("lib.op").is_file() {
        return Some(sibling);
    }
    None
}

/// Parse and compile a source string against the std library, returning
/// the object file and the codegen name tables.
fn compile_std(src: &str) -> Option<(ObjectFile, NameTables)> {
    let root = std_root()?;
    let (ast, _diags) = parse_source("test.op", src, "rp2A03-nintendo-nes-ntsc", &[]);
    let includes = vec![root.to_string_lossy().into_owned()];
    let (obj, _diags, tables) = compile_source_with_tables(&ast, 0, &includes, &[]);
    Some((obj, tables))
}

/// Parse a std source file directly (as the root source) and compile it,
/// returning the object file and the codegen name tables.
fn compile_std_file(path: &std::path::Path) -> Option<(ObjectFile, NameTables)> {
    let source = std::fs::read_to_string(path).ok()?;
    let file = path.to_string_lossy().into_owned();
    let (ast, _diags) = parse_source(&file, &source, "rp2A03-nintendo-nes-ntsc", &[]);
    let (obj, _diags, tables) = compile_source_with_tables(&ast, 0, &[], &[]);
    Some((obj, tables))
}

#[test]
fn std_resolves_cpu_glob() {
    let Some((_obj, tables)) = compile_std("use std::cpu::*;") else {
        eprintln!("skipping: std library not found (set OP_STD_PATH)");
        return;
    };
    // The cpu module exports no inline fns.
    assert!(tables.inline_fn_names.is_empty());
    // Enum variants land in the flat const namespace under qualified keys.
    assert_eq!(tables.const_values.get("STATUS::N"), Some(&0x80));
    assert_eq!(tables.const_values.get("STATUS::C"), Some(&0x01));
    assert_eq!(tables.const_values.get("CPU_REG::a"), Some(&0));
    assert_eq!(tables.const_values.get("CPU_REG::y"), Some(&2));
    // Implicit variant values are counted from the previous variant.
    assert_eq!(tables.const_values.get("OPCODE::BRK"), Some(&10));
    // The module's `pub use CPU_REG::*;` re-exports the register names bare.
    assert_eq!(tables.const_values.get("a"), Some(&0));
    assert_eq!(tables.const_values.get("y"), Some(&2));
}

#[test]
fn std_resolves_machine_macros() {
    let Some((_obj, tables)) = compile_std("use std::machine::*;") else {
        eprintln!("skipping: std library not found (set OP_STD_PATH)");
        return;
    };
    for name in ["system_initialize", "vram_write", "turn_video_on"] {
        assert!(
            tables.inline_fn_names.contains(&name.to_string()),
            "missing inline fn `{name}`"
        );
    }
}

#[test]
fn std_enum_variant_resolves() {
    let Some((obj, _tables)) = compile_std(
        "use std::machine::*;
         #[rom(org = 0, bank = 0, maxsize = 0x100)] { fn f() { lda #COLOUR::YELLOW } }",
    ) else {
        eprintln!("skipping: std library not found (set OP_STD_PATH)");
        return;
    };
    let data = &obj.sections[0].data;
    // LDA immediate with the resolved variant value: A9 07.
    assert_eq!(data[0], 0xA9);
    assert_eq!(data[1], 0x07);
    assert!(obj.sections[0].relocations.is_empty());
}

#[test]
fn std_inline_fn_param_substitution() {
    let Some((obj, _tables)) = compile_std(
        "use std::machine::*;
         #[rom(org = 0, bank = 0, maxsize = 0x100)] { fn f() { vram_write(0x30) } }",
    ) else {
        eprintln!("skipping: std library not found (set OP_STD_PATH)");
        return;
    };
    let data = &obj.sections[0].data;
    // vram_write(value) expands to `lda #value; sta PPU::IO`. With
    // value = 0x30 the load is immediate and PPU::IO resolves to
    // 0x2007: LDA #$30 (A9 30) + STA $2007 (8D 07 20).
    assert_eq!(&data[..5], &[0xA9, 0x30, 0x8D, 0x07, 0x20]);
    assert!(obj.sections[0].relocations.is_empty());
}

#[test]
fn std_super_resolution() {
    let Some(root) = std_root() else {
        eprintln!("skipping: std library not found (set OP_STD_PATH)");
        return;
    };
    let path = root.join("machine").join("nes").join("macros.op");
    let Some((_obj, tables)) = compile_std_file(&path) else {
        eprintln!("skipping: {} not readable", path.display());
        return;
    };
    // macros.op's `use super::types::*;` resolves against its own
    // directory and imports the PPU register enum.
    assert_eq!(tables.const_values.get("PPU::CNT0"), Some(&0x2000));
}

// --- rp2A03 / rp2A07 CPU tests -----------------------------------------------

#[test]
fn rp2a03_encoding_table_is_6502() {
    use opc::encoding::get_encoding_table;
    let table = get_encoding_table("rp2A03");
    assert!(!table.is_empty());
    assert_eq!(table.len(), opc::encoding::ENCODING_6502.len());
}

#[test]
fn rp2a07_encoding_table_is_6502() {
    use opc::encoding::get_encoding_table;
    let table = get_encoding_table("rp2A07");
    assert!(!table.is_empty());
    assert_eq!(table.len(), opc::encoding::ENCODING_6502.len());
}

#[test]
fn rp2a03_full_encoding_table_has_lda_immediate() {
    use opc::encoding::get_full_encoding_table;
    let table = get_full_encoding_table("rp2A03");
    assert!(table.iter().any(|e| e.mnemonic.eq_ignore_ascii_case("lda")
        && matches!(e.mode, opc::encoding::AddrMode::Immediate)));
}

#[test]
fn rp2a07_full_encoding_table_has_lda_immediate() {
    use opc::encoding::get_full_encoding_table;
    let table = get_full_encoding_table("rp2A07");
    assert!(table.iter().any(|e| e.mnemonic.eq_ignore_ascii_case("lda")
        && matches!(e.mode, opc::encoding::AddrMode::Immediate)));
}

#[test]
fn rp2a03_interrupt_vector_reset() {
    use opc::codegen::interrupt_vector_address;
    assert_eq!(interrupt_vector_address("rp2A03", "reset"), Some(0xFFFC));
}

#[test]
fn rp2a03_interrupt_vector_nmi() {
    use opc::codegen::interrupt_vector_address;
    assert_eq!(interrupt_vector_address("rp2A03", "nmi"), Some(0xFFFA));
}

#[test]
fn rp2a03_interrupt_vector_irq() {
    use opc::codegen::interrupt_vector_address;
    assert_eq!(interrupt_vector_address("rp2A03", "irq"), Some(0xFFFE));
}

#[test]
fn rp2a07_interrupt_vector_reset() {
    use opc::codegen::interrupt_vector_address;
    assert_eq!(interrupt_vector_address("rp2A07", "reset"), Some(0xFFFC));
}

#[test]
fn rp2a07_interrupt_vector_nmi() {
    use opc::codegen::interrupt_vector_address;
    assert_eq!(interrupt_vector_address("rp2A07", "nmi"), Some(0xFFFA));
}

#[test]
fn rp2a07_interrupt_vector_irq() {
    use opc::codegen::interrupt_vector_address;
    assert_eq!(interrupt_vector_address("rp2A07", "irq"), Some(0xFFFE));
}
