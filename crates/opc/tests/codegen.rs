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

// === Interrupt vector address lookup ======================================

#[test]
fn vector_address_6502_family() {
    use opc::codegen::interrupt_vector_address;
    assert_eq!(interrupt_vector_address("mos6502", "reset"), Some(0xFFFC));
    assert_eq!(interrupt_vector_address("mos6502", "nmi"), Some(0xFFFA));
    assert_eq!(interrupt_vector_address("mos6502", "irq"), Some(0xFFF8));
    assert_eq!(interrupt_vector_address("rp2A03", "reset"), Some(0xFFFC));
    assert_eq!(interrupt_vector_address("vl65NC02", "irq"), Some(0xFFF8));
}

#[test]
fn vector_address_65c816() {
    use opc::codegen::interrupt_vector_address;
    assert_eq!(interrupt_vector_address("wdc65c816", "reset"), Some(0xFFFC));
    assert_eq!(interrupt_vector_address("wdc65c816", "nmi"), Some(0xFFEA));
    assert_eq!(interrupt_vector_address("wdc65c816", "irq"), Some(0xFFEE));
    assert_eq!(interrupt_vector_address("wdc65c816", "abort"), Some(0xFFE8));
    assert_eq!(interrupt_vector_address("wdc65c816", "cop"), Some(0xFFE4));
}

#[test]
fn vector_address_sm83() {
    use opc::codegen::interrupt_vector_address;
    assert_eq!(interrupt_vector_address("sm83", "vblank"), Some(0x0040));
    assert_eq!(interrupt_vector_address("sm83", "lcdc"), Some(0x0048));
    assert_eq!(interrupt_vector_address("sm83", "timer"), Some(0x0050));
    assert_eq!(interrupt_vector_address("sm83", "serial"), Some(0x0058));
    assert_eq!(interrupt_vector_address("sm83", "joypad"), Some(0x0060));
}

#[test]
fn vector_address_z80() {
    use opc::codegen::interrupt_vector_address;
    assert_eq!(interrupt_vector_address("z80", "reset"), Some(0x0000));
    assert_eq!(interrupt_vector_address("z80", "rst8"), Some(0x0008));
    assert_eq!(interrupt_vector_address("z80", "rst10"), Some(0x0010));
    assert_eq!(interrupt_vector_address("z80", "rst18"), Some(0x0018));
    assert_eq!(interrupt_vector_address("z80", "rst20"), Some(0x0020));
    assert_eq!(interrupt_vector_address("z80", "rst28"), Some(0x0028));
    assert_eq!(interrupt_vector_address("z80", "rst30"), Some(0x0030));
    assert_eq!(interrupt_vector_address("z80", "rst38"), Some(0x0038));
    assert_eq!(interrupt_vector_address("z80", "irq"), Some(0x0038));
    assert_eq!(interrupt_vector_address("z80", "nmi"), Some(0x0066));
}

#[test]
fn vector_address_68000() {
    use opc::codegen::interrupt_vector_address;
    assert_eq!(interrupt_vector_address("m68000", "reset"), Some(0x0000));
    assert_eq!(interrupt_vector_address("m68000", "reset_pc"), Some(0x0004));
    assert_eq!(
        interrupt_vector_address("m68000", "bus_error"),
        Some(0x0008)
    );
    assert_eq!(
        interrupt_vector_address("m68000", "address_error"),
        Some(0x000C)
    );
    assert_eq!(interrupt_vector_address("m68000", "illegal"), Some(0x0010));
    assert_eq!(
        interrupt_vector_address("m68000", "zero_divide"),
        Some(0x0014)
    );
    assert_eq!(interrupt_vector_address("m68000", "chk"), Some(0x0018));
    assert_eq!(interrupt_vector_address("m68000", "trapv"), Some(0x001C));
    assert_eq!(
        interrupt_vector_address("m68000", "privilege"),
        Some(0x0020)
    );
    assert_eq!(interrupt_vector_address("m68000", "trace"), Some(0x0024));
    assert_eq!(interrupt_vector_address("m68000", "line_a"), Some(0x0028));
    assert_eq!(interrupt_vector_address("m68000", "line_f"), Some(0x002C));
    assert_eq!(interrupt_vector_address("m68000", "spurious"), Some(0x0060));
    assert_eq!(interrupt_vector_address("m68000", "level1"), Some(0x0064));
    assert_eq!(interrupt_vector_address("m68000", "level2"), Some(0x0068));
    assert_eq!(interrupt_vector_address("m68000", "level3"), Some(0x006C));
    assert_eq!(interrupt_vector_address("m68000", "level4"), Some(0x0070));
    assert_eq!(interrupt_vector_address("m68000", "level5"), Some(0x0074));
    assert_eq!(interrupt_vector_address("m68000", "level6"), Some(0x0078));
    assert_eq!(interrupt_vector_address("m68000", "level7"), Some(0x007C));
    assert_eq!(interrupt_vector_address("m68000", "trap0"), Some(0x0080));
    assert_eq!(interrupt_vector_address("m68000", "trap15"), Some(0x00BC));
}

#[test]
fn vector_encoding_for_cpu() {
    use op_ir::VectorEncoding;
    use opc::codegen::vector_encoding_for;
    assert_eq!(vector_encoding_for("mos6502"), VectorEncoding::Pointer2);
    assert_eq!(vector_encoding_for("rp2A03"), VectorEncoding::Pointer2);
    assert_eq!(vector_encoding_for("wdc65c816"), VectorEncoding::Pointer2);
    assert_eq!(vector_encoding_for("sm83"), VectorEncoding::JumpSm83);
    assert_eq!(vector_encoding_for("z80"), VectorEncoding::JumpZ80);
    assert_eq!(vector_encoding_for("m68000"), VectorEncoding::Pointer4);
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
    // The cpu module now exports inline fns from the 6502 macros.
    assert!(tables.inline_fn_names.contains(&"assign".to_string()));
    assert!(tables.inline_fn_names.contains(&"pusha".to_string()));
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
    assert_eq!(interrupt_vector_address("rp2A03", "irq"), Some(0xFFF8));
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
    assert_eq!(interrupt_vector_address("rp2A07", "irq"), Some(0xFFF8));
}

// === W65C02 CPU tests -------------------------------------------------------

#[test]
fn w65c02_encoding_table_superset_of_65sc02() {
    use opc::encoding::{get_encoding_table, get_full_encoding_table};
    let base = get_encoding_table("w65c02");
    assert!(!base.is_empty());
    // The base table lists only the opcodes beyond the 65SC02 core.
    assert_eq!(base.len(), opc::encoding::ENCODING_W65C02.len());

    let full = get_full_encoding_table("w65c02");
    // Base 6502 + 65SC02 + W65C02 tables.
    assert_eq!(
        full.len(),
        opc::encoding::ENCODING_6502.len()
            + opc::encoding::ENCODING_65SC02.len()
            + opc::encoding::ENCODING_W65C02.len()
    );
}

#[test]
fn w65c02_full_table_has_65sc02_and_w65c02_entries() {
    use opc::encoding::{get_full_encoding_table, AddrMode};
    let table = get_full_encoding_table("w65c02");
    // 65SC02 core entry present (BRA relative).
    assert!(table.iter().any(|e| e.mnemonic.eq_ignore_ascii_case("bra")
        && matches!(e.mode, AddrMode::Relative)
        && e.opcode == 0x80));
    // WDC low-power modes.
    assert!(table
        .iter()
        .any(|e| e.mnemonic.eq_ignore_ascii_case("wai") && e.opcode == 0xCB));
    assert!(table
        .iter()
        .any(|e| e.mnemonic.eq_ignore_ascii_case("stp") && e.opcode == 0xDB));
    // Rockwell bit manipulation.
    assert!(table
        .iter()
        .any(|e| e.mnemonic.eq_ignore_ascii_case("smb0") && e.opcode == 0x87));
    assert!(table
        .iter()
        .any(|e| e.mnemonic.eq_ignore_ascii_case("rmb7") && e.opcode == 0x77));
    assert!(table
        .iter()
        .any(|e| e.mnemonic.eq_ignore_ascii_case("bbr0") && e.opcode == 0x0F));
    assert!(table
        .iter()
        .any(|e| e.mnemonic.eq_ignore_ascii_case("bbs7") && e.opcode == 0xFF));
}

/// Helper: parse and compile a source string with the W65C02 target.
fn compile_w65c02(src: &str) -> ObjectFile {
    let (ast, _diags) = parse_source("test.op", src, "w65c02-commander-x16", &[]);
    let (obj, _codegen_diags) = compile_source(&ast, 1, &[], &[]);
    obj
}

#[test]
fn w65c02_target_uses_w65c02_encodings() {
    let obj =
        compile_w65c02("#[rom(org = 0xC000, bank = 32, maxsize = 0x4000)] { fn main() { wai } }");
    let rom = obj.sections.iter().find(|s| s.kind == SectionKind::Rom);
    assert!(rom.is_some());
    // WAI assembles to the single byte 0xCB, followed by the implicit
    // RTS the codegen appends to a fn that does not end control flow.
    assert_eq!(rom.unwrap().data, vec![0xCB, 0x60]);
}

#[test]
fn w65c02_smb0_emits_zeropage_form() {
    let obj = compile_w65c02(
        "#[rom(org = 0xC000, bank = 32, maxsize = 0x4000)] { fn main() { smb0 0x20 } }",
    );
    let rom = obj.sections.iter().find(|s| s.kind == SectionKind::Rom);
    // SMB0 $20 -> 87 20, then the implicit RTS.
    assert_eq!(rom.unwrap().data, vec![0x87, 0x20, 0x60]);
}

#[test]
fn w65c02_bbs0_branches_to_label() {
    let obj = compile_w65c02(
        "#[rom(org = 0xC000, bank = 32, maxsize = 0x4000)] {
            fn main() {
                bbs0 0x20, 'skip
                nop
                'skip: rts
            }
        }",
    );
    let rom = obj.sections.iter().find(|s| s.kind == SectionKind::Rom);
    let rom = rom.unwrap();
    // BBS0 $20,'skip -> 8F 20 <placeholder offset>.
    assert_eq!(rom.data[0], 0x8F);
    assert_eq!(rom.data[1], 0x20);
    // The branch offset is a Branch8 relocation against the 'skip
    // label at the third instruction byte. The linker computes the
    // relative displacement.
    let branch = rom
        .relocations
        .iter()
        .find(|r| r.kind == op_ir::RelocKind::Branch8 && r.symbol == "skip");
    assert!(
        branch.is_some(),
        "no Branch8 reloc for 'skip: {:?}",
        rom.relocations
    );
    let branch = branch.unwrap();
    assert_eq!(branch.offset, 2);
    assert_eq!(rom.data[3], 0xEA); // NOP
    assert_eq!(rom.data[4], 0x60); // RTS at the 'skip label
}

// === Phase 0: array const placement ========================================

#[test]
fn array_const_placed_in_rom() {
    let obj = compile(
        "#[rom(org = 0xC000, bank = 0, maxsize = 0x4000)] {
            fn main() { lda DATA }
        }
         const DATA: [u8; 4] = [0x01, 0x02, 0x03, 0x04];",
    );
    // The rom section should exist and contain data.
    let rom = obj.sections.iter().find(|s| s.kind == SectionKind::Rom);
    assert!(rom.is_some());
    let rom = rom.unwrap();
    // Find the DATA symbol.
    let sym = rom.symbols.iter().find(|s| s.name == "DATA");
    assert!(
        sym.is_some(),
        "DATA symbol not found in symbols: {:?}",
        rom.symbols
    );
    let sym = sym.unwrap();
    let start = sym.offset as usize;
    let end = start + 4;
    let bytes = &rom.data[start..end];
    assert_eq!(bytes, &[0x01, 0x02, 0x03, 0x04]);
}

#[test]
fn array_const_len_resolves() {
    let (ast, _diags) = parse_source(
        "test.op",
        "const DATA: [u8; 4] = [10, 20, 30, 40];",
        "rp2A03-nintendo-nes-ntsc",
        &[],
    );
    let (obj, _diags, tables) = compile_source_with_tables(&ast, 1, &[], &[]);
    // len!(DATA) should resolve to 4 via const_values.
    // We verify via the NameTables.
    let data_len = tables.const_values.get("DATA");
    assert_eq!(data_len, Some(&4));
    let _ = obj;
}

// === Phase 0: struct const field resolution ================================

#[test]
fn struct_const_scalar_field_in_const_values() {
    let (ast, _diags) = parse_source(
        "test.op",
        "#[rom(org = 0xC000, bank = 0, maxsize = 0x4000)] { }
         const MYFONT: font_t = font_t { tile_count: 102, data: SOMEDATA };",
        "rp2A03-nintendo-nes-ntsc",
        &[],
    );
    let (obj, _diags, tables) = compile_source_with_tables(&ast, 1, &[], &[]);
    // The scalar field tile_count should be in const_values as
    // MYFONT::tile_count = 102.
    let tile_count = tables.const_values.get("MYFONT::tile_count");
    assert_eq!(tile_count, Some(&102));
    let _ = obj;
}
