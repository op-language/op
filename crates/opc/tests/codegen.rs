//! Codegen and optimizer unit tests.
//!
//! These tests call `opc::codegen::compile_source()` on parsed ASTs and
//! assert the `ObjectFile` structure (sections, symbols, relocations,
//! data bytes).

use op_ir::{ObjectFile, SectionKind, SymbolKind};
use opc::codegen::compile_source;
use opc::parser::parse_source;

/// Helper: parse and compile a source string with the 6502 target.
fn compile(src: &str) -> ObjectFile {
    let (ast, _diags) = parse_source("test.op", src, "mos6502-nintendo-nes-ntsc", &[]);
    let (obj, _codegen_diags) = compile_source(&ast, 1);
    obj
}

/// Helper: parse and compile with a specific opt level.
fn compile_with_opt(src: &str, opt_level: u8) -> ObjectFile {
    let (ast, _diags) = parse_source("test.op", src, "mos6502-nintendo-nes-ntsc", &[]);
    let (obj, _) = compile_source(&ast, opt_level);
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
    assert_eq!(data.len(), 1);
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
    assert_eq!(data.len(), 2);
}

#[test]
fn codegen_absolute_mode() {
    let obj = compile("#[rom(org = 0, bank = 0, maxsize = 0x100)] { fn f() { lda 0x2000 } }");
    let data = &obj.sections[0].data;
    assert_eq!(data[0], 0xAD); // LDA absolute
    assert_eq!(data.len(), 3);
}

#[test]
fn codegen_forced_zp_mode() {
    let obj = compile("#[rom(org = 0, bank = 0, maxsize = 0x100)] { fn f() { lda zp 0x2000 } }");
    let data = &obj.sections[0].data;
    // Forced zero-page should use zero-page encoding even for larger addresses.
    assert_eq!(data[0], 0xA5); // LDA zero-page
    assert_eq!(data[1], 0x00); // truncated to 1 byte
    assert_eq!(data.len(), 2);
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
    let src = "#[rom(org = 0, bank = 0, maxsize = 0x100)] { fn f() { sta 0x2000 sta 0x2000 inx } }";
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

// === Other CPU families ===================================================

#[test]
fn codegen_z80_target() {
    let (ast, _) = parse_source("test.op", "fn f() { nop }", "z80-nintendo-gameboy", &[]);
    let (obj, _) = compile_source(&ast, 1);
    // Z80 should produce at least an empty or minimal output.
    assert_eq!(obj.target, "z80-nintendo-gameboy");
}

#[test]
fn codegen_68000_target() {
    let (ast, _) = parse_source("test.op", "fn f() { nop }", "m68000-sega-genesis", &[]);
    let (obj, _) = compile_source(&ast, 1);
    assert_eq!(obj.target, "m68000-sega-genesis");
}

#[test]
fn codegen_65c816_target() {
    let (ast, _) = parse_source("test.op", "fn f() { nop }", "wdc65c816-nintendo-snes", &[]);
    let (obj, _) = compile_source(&ast, 1);
    assert_eq!(obj.target, "wdc65c816-nintendo-snes");
}

// === Empty source =========================================================

#[test]
fn codegen_empty_source() {
    let (ast, _) = parse_source("test.op", "", "mos6502-nintendo-nes-ntsc", &[]);
    let (obj, _) = compile_source(&ast, 1);
    assert_eq!(obj.sections.len(), 0);
}

#[test]
fn codegen_no_block_attributes() {
    // Functions without a #[rom] block produce no sections.
    let obj = compile("fn f() { lda 0 }");
    assert_eq!(obj.sections.len(), 0);
}
