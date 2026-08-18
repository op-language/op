//! Integration tests: lexer + parser together.
//!
//! These tests run the lexer and then the parser on real Op source files
//! and assert the resulting AST is well-formed.

use opc::parser::parse_source;

#[test]
fn lex_then_parse_nes_code() {
    let source = include_str!("../../../examples/nes.op");
    let (ast, diags) = parse_source("examples/nes.op", source, "mos6502-nintendo-nes-ntsc", &[]);

    // No error diagnostics.
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| d.severity == op_diagnostics::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "parser errors: {:?}", errors);

    let items = &ast.root.items;
    assert!(
        items.len() >= 15,
        "expected at least 15 items, got {}",
        items.len()
    );

    // Should have use declarations.
    let use_count = items
        .iter()
        .filter(|i| matches!(i, op_common::ast::Item::UseDecl { .. }))
        .count();
    assert!(
        use_count >= 2,
        "expected at least 2 use decls, got {}",
        use_count
    );

    // Should have fn declarations.
    let fn_count = items
        .iter()
        .filter(|i| matches!(i, op_common::ast::Item::FnDecl { .. }))
        .count();
    assert!(
        fn_count >= 5,
        "expected at least 5 fn decls, got {}",
        fn_count
    );

    // Should have inline fn declarations.
    let inline_count = items
        .iter()
        .filter(|i| matches!(i, op_common::ast::Item::InlineFnDecl { .. }))
        .count();
    assert!(
        inline_count >= 3,
        "expected at least 3 inline fn decls, got {}",
        inline_count
    );

    // Should have a var declaration (str_hello).
    let var_count = items
        .iter()
        .filter(|i| matches!(i, op_common::ast::Item::VarDecl { .. }))
        .count();
    assert!(
        var_count >= 1,
        "expected at least 1 var decl, got {}",
        var_count
    );
}

#[test]
fn lex_then_parse_nes_game() {
    let source = include_str!("../../../examples/nes.op");
    let (ast, diags) = parse_source("examples/nes.op", source, "mos6502-nintendo-nes-ntsc", &[]);

    let errors: Vec<_> = diags
        .iter()
        .filter(|d| d.severity == op_diagnostics::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "parser errors: {:?}", errors);

    let items = &ast.root.items;
    assert!(
        items.len() >= 15,
        "expected at least 15 items, got {}",
        items.len()
    );

    // Should have use declarations.
    let has_use = items
        .iter()
        .any(|i| matches!(i, op_common::ast::Item::UseDecl { .. }));
    assert!(has_use, "expected a UseDecl");

    // Should have block attributes (rom, ram, chr, ines, setpad).
    let block_count = items
        .iter()
        .filter(|i| matches!(i, op_common::ast::Item::BlockAttribute { .. }))
        .count();
    assert!(
        block_count >= 5,
        "expected at least 5 block attributes, got {}",
        block_count
    );
}

#[test]
fn lex_then_parse_std_mos6502() {
    let source = include_str!("../../../../std/src/cpu/mos6502.op");
    let (ast, diags) = parse_source(
        "std/src/cpu/mos6502.op",
        source,
        "mos6502-nintendo-nes-ntsc",
        &[],
    );

    let errors: Vec<_> = diags
        .iter()
        .filter(|d| d.severity == op_diagnostics::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "parser errors: {:?}", errors);

    let items = &ast.root.items;
    assert!(
        items.len() >= 7,
        "expected at least 7 items, got {}",
        items.len()
    );

    // Should have enum declarations.
    let enum_count = items
        .iter()
        .filter(|i| matches!(i, op_common::ast::Item::EnumDecl { .. }))
        .count();
    assert!(
        enum_count >= 5,
        "expected at least 5 enum decls, got {}",
        enum_count
    );

    // Should have use declarations (pub use re-exports).
    let use_count = items
        .iter()
        .filter(|i| matches!(i, op_common::ast::Item::UseDecl { .. }))
        .count();
    assert!(
        use_count >= 5,
        "expected at least 5 use decls, got {}",
        use_count
    );
}

#[test]
fn lex_then_parse_std_nes_macros() {
    let source = include_str!("../../../../std/src/machine/nes/macros.op");
    let (ast, diags) = parse_source(
        "std/src/machine/nes/macros.op",
        source,
        "mos6502-nintendo-nes-ntsc",
        &[],
    );

    let errors: Vec<_> = diags
        .iter()
        .filter(|d| d.severity == op_diagnostics::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "parser errors: {:?}", errors);

    let items = &ast.root.items;
    assert!(
        items.len() >= 5,
        "expected at least 5 items, got {}",
        items.len()
    );

    // Should have use declarations (super:: imports).
    let use_count = items
        .iter()
        .filter(|i| matches!(i, op_common::ast::Item::UseDecl { .. }))
        .count();
    assert!(
        use_count >= 2,
        "expected at least 2 use decls, got {}",
        use_count
    );

    // Should have inline fn declarations.
    let inline_count = items
        .iter()
        .filter(|i| matches!(i, op_common::ast::Item::InlineFnDecl { .. }))
        .count();
    assert!(
        inline_count >= 5,
        "expected at least 5 inline fn decls, got {}",
        inline_count
    );
}

#[test]
fn full_pipeline_lex_then_parse() {
    // Verify that the full lex+parse pipeline works on a source with
    // all major constructs.
    let src = r#"
        use std::cpu::*;
        use std::machine::*;

        const SCREEN_WIDTH: u8 = 256;

        #[ram(org = 0x0000, maxsize = 0x100)] {
            counter: u8;
        }

        #[rom(org = 0xC000, bank = 0)] {
            #[interrupt(reset)]
            locate_fn!(game::main);
        }

        fn main() {
            lda 0
            sta PPU::CNT0
            if (set) {
                lda 1
            } else {
                lda 2
            }
            loop {
                wait_for(6)
            }
        }

        inline fn wait_for(amount) {
            ldx amount
        }
    "#;

    let (ast, diags) = parse_source("test.op", src, "mos6502-nintendo-nes-ntsc", &[]);

    let errors: Vec<_> = diags
        .iter()
        .filter(|d| d.severity == op_diagnostics::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "parser errors: {:?}", errors);

    let items = &ast.root.items;
    assert!(
        items.len() >= 6,
        "expected at least 6 items, got {}",
        items.len()
    );
}

#[test]
fn parse_with_cfg_filtering() {
    let src = r#"
        #[cfg(cpu = "mos6502")]
        const ON_6502: u8 = 1;

        #[cfg(cpu = "z80")]
        const ON_Z80: u8 = 2;

        const ALWAYS: u8 = 3;
    "#;

    let (ast, _diags) = parse_source("test.op", src, "mos6502-nintendo-nes-ntsc", &[]);
    let items = &ast.root.items;

    // ON_6502 and ALWAYS should be present; ON_Z80 should be dropped.
    assert_eq!(items.len(), 2);

    let names: Vec<&str> = items
        .iter()
        .filter_map(|i| match i {
            op_common::ast::Item::ConstDecl { name, .. } => Some(name.as_str()),
            _ => None,
        })
        .collect();
    assert!(names.contains(&"ON_6502"));
    assert!(names.contains(&"ALWAYS"));
    assert!(!names.contains(&"ON_Z80"));
}

#[test]
fn parse_with_feature_flag() {
    let src = r#"
        #[cfg(feature = "undocumented")]
        const HAS_UNDOC: u8 = 1;

        const ALWAYS: u8 = 2;
    "#;

    // Without the feature flag, HAS_UNDOC should be dropped.
    let (ast, _) = parse_source("test.op", src, "mos6502-nintendo-nes-ntsc", &[]);
    assert_eq!(ast.root.items.len(), 1);

    // With the feature flag, HAS_UNDOC should be present.
    let (ast, _) = parse_source(
        "test.op",
        src,
        "mos6502-nintendo-nes-ntsc",
        &["undocumented".to_string()],
    );
    assert_eq!(ast.root.items.len(), 2);
}

// === Lexer + Parser + Codegen integration tests ============================

use opc::codegen::compile_source;

#[test]
fn lex_parse_compile_nes_code() {
    let source = include_str!("../../../examples/nes.op");
    let (ast, parse_diags) =
        parse_source("examples/nes.op", source, "mos6502-nintendo-nes-ntsc", &[]);

    let errors: Vec<_> = parse_diags
        .iter()
        .filter(|d| d.severity == op_diagnostics::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "parser errors: {:?}", errors);

    let (obj, _codegen_diags) = compile_source(&ast, 1);

    // nes.op has block attributes (rom, ram, chr, ines, setpad).
    // The codegen should not crash. The font.chr locate_bytes may
    // produce an error if the CWD doesn't contain font.chr, but the
    // codegen should still produce sections.
    assert_eq!(obj.target, "mos6502-nintendo-nes-ntsc");
    assert!(!obj.sections.is_empty(), "expected sections from nes.op");
}

#[test]
fn lex_parse_compile_nes_game() {
    let source = include_str!("../../../examples/nes.op");
    let (ast, parse_diags) =
        parse_source("examples/nes.op", source, "mos6502-nintendo-nes-ntsc", &[]);

    let errors: Vec<_> = parse_diags
        .iter()
        .filter(|d| d.severity == op_diagnostics::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "parser errors: {:?}", errors);

    let (obj, _codegen_diags) = compile_source(&ast, 1);
    let has_sections = !obj.sections.is_empty();

    // nes.op has #[rom], #[ram], #[chr] blocks — should produce sections.
    assert!(has_sections, "expected at least 1 section");

    // Check that we have ROM and RAM sections.
    let has_rom = obj
        .sections
        .iter()
        .any(|s| s.kind == op_ir::SectionKind::Rom);
    let has_ram = obj
        .sections
        .iter()
        .any(|s| s.kind == op_ir::SectionKind::Ram);
    assert!(has_rom, "expected a ROM section");
    assert!(has_ram, "expected a RAM section");
}

#[test]
fn full_pipeline_lex_parse_compile() {
    let src = r#"
        #[rom(org = 0xC000, bank = 0, maxsize = 0x4000)] {
            fn main() {
                lda 0
                sta 0x2000
                inx
                loop {
                    inx
                }
                return
            }
        }

        #[ram(org = 0x0000, maxsize = 0x100)] {
            counter: u8;
            flag: u8 = 0;
        }
    "#;

    let (ast, parse_diags) = parse_source("test.op", src, "mos6502-nintendo-nes-ntsc", &[]);
    let errors: Vec<_> = parse_diags
        .iter()
        .filter(|d| d.severity == op_diagnostics::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "parser errors: {:?}", errors);

    let (obj, codegen_diags) = compile_source(&ast, 1);
    let errors: Vec<_> = codegen_diags
        .iter()
        .filter(|d| d.severity == op_diagnostics::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "codegen errors: {:?}", errors);

    assert_eq!(obj.sections.len(), 2);

    // ROM section should have data (encoded instructions).
    let rom = &obj.sections[0];
    assert_eq!(rom.kind, op_ir::SectionKind::Rom);
    assert!(!rom.data.is_empty(), "ROM section should have encoded data");

    // ROM section should have a 'main' function symbol.
    assert!(rom.symbols.iter().any(|s| s.name == "main"));

    // RAM section should have variable symbols.
    let ram = &obj.sections[1];
    assert_eq!(ram.kind, op_ir::SectionKind::Ram);
    assert!(ram.symbols.iter().any(|s| s.name == "counter"));
    assert!(ram.symbols.iter().any(|s| s.name == "flag"));
}
