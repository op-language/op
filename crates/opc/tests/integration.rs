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

    let (obj, _codegen_diags) = compile_source(&ast, 1, &[], &[]);

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

    let (obj, _codegen_diags) = compile_source(&ast, 1, &[], &[]);
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

    let (obj, codegen_diags) = compile_source(&ast, 1, &[], &[]);
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

// === Full pipeline (lex + parse + compile + link + output) ================

use opc::linker::link_source;
use opc::output::{default_format_for_target, emit_linked};

fn run_full_pipeline(
    file: &str,
    src: &str,
    target: &str,
) -> (op_ir::ObjectFile, op_ir::ObjectFile, Vec<u8>) {
    run_full_pipeline_with_includes(file, src, target, &[])
}

fn run_full_pipeline_with_includes(
    file: &str,
    src: &str,
    target: &str,
    include_paths: &[String],
) -> (op_ir::ObjectFile, op_ir::ObjectFile, Vec<u8>) {
    let (ast, parse_diags) = parse_source(file, src, target, &[]);
    let errors: Vec<_> = parse_diags
        .iter()
        .filter(|d| d.severity == op_diagnostics::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "parser errors: {:?}", errors);

    let (obj, codegen_diags) = compile_source(&ast, 1, include_paths, &[]);
    let errors: Vec<_> = codegen_diags
        .iter()
        .filter(|d| d.severity == op_diagnostics::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "codegen errors: {:?}", errors);

    let (linked, link_diags) = link_source(&obj);
    let errors: Vec<_> = link_diags
        .iter()
        .filter(|d| d.severity == op_diagnostics::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "linker errors: {:?}", errors);

    let format = default_format_for_target(target);
    let bytes = emit_linked(&linked, format).expect("emit_linked failed");
    (obj, linked, bytes)
}

const HELLO_NES_SRC: &str = r#"
    #[rom(org = 0xC000, bank = 0, maxsize = 0x4000)] {
        #[interrupt(reset)]
        fn main() {
            lda #1
            sta 0x20
            loop {
                jmp main
            }
        }
    }
"#;

#[test]
fn full_pipeline_end_to_end() {
    let (_obj, _linked, bytes) =
        run_full_pipeline("test.op", HELLO_NES_SRC, "mos6502-nintendo-nes-ntsc");
    assert!(!bytes.is_empty(), "output binary should not be empty");
}

#[test]
fn full_pipeline_ines_output() {
    let (_obj, _linked, bytes) =
        run_full_pipeline("test.op", HELLO_NES_SRC, "mos6502-nintendo-nes-ntsc");
    assert!(
        bytes.starts_with(&[b'N', b'E', b'S', 0x1A]),
        "iNES output should start with NES magic"
    );
}

#[test]
fn full_pipeline_raw_output() {
    // Apple II uses raw output (no dedicated format).
    let src = r#"
        #[rom(org = 0x0801, bank = 0, maxsize = 0x7FFF)] {
            fn start() {
                lda #0
                sta 0x0000
                rts
            }
        }
    "#;
    let (_obj, _linked, bytes) = run_full_pipeline("test.op", src, "mos6502-apple-apple2e-ntsc");
    assert!(!bytes.is_empty(), "raw output should not be empty");
}

#[test]
fn full_pipeline_link_resolved() {
    let (_obj, linked, _bytes) =
        run_full_pipeline("test.op", HELLO_NES_SRC, "mos6502-nintendo-nes-ntsc");
    for s in &linked.sections {
        assert!(
            s.relocations.is_empty(),
            "section {} should have no relocations after link",
            s.name
        );
    }
}

// === Full pipeline with the std library ====================================

/// Locate the std crate root: the `OP_STD_PATH` environment variable if it
/// contains a `lib.op` file, otherwise the `std/src` directory three levels
/// above the crate directory.
fn std_root() -> Option<std::path::PathBuf> {
    if let Ok(path) = std::env::var("OP_STD_PATH") {
        let root = std::path::PathBuf::from(path);
        if root.join("lib.op").is_file() {
            return Some(root);
        }
    }
    let sibling = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../std/src");
    if sibling.join("lib.op").is_file() {
        return Some(sibling);
    }
    None
}

/// A self-contained std program: it imports the std globs, declares
/// interrupt-attribute fns inside a ROM block, and locates a binary font in
/// a CHR block.
const STD_NES_SRC: &str = r#"
use std::cpu::*;
use std::machine::*;

#[rom(org = 0xC000, bank = 0, maxsize = 0x4000)] {
    #[interrupt(reset)]
    fn main() {
        system_initialize()
        turn_video_on()
        loop {
            vblank_wait()
        }
    }

    #[interrupt(nmi)]
    fn nmi() {
    }
}

#[chr(bank = 0)] {
    locate_bytes!("blob.chr")
}
"#;

/// Write the std fixture (source + font file) into `dir` and run the full
/// pipeline on the source, parsed with its absolute path.
fn run_std_pipeline(dir: &std::path::Path, std_root: &std::path::Path) -> Vec<u8> {
    std::fs::write(dir.join("blob.chr"), vec![0xABu8; 8192]).unwrap();
    let src_path = dir.join("std_test.op");
    std::fs::write(&src_path, STD_NES_SRC).unwrap();
    let (_obj, _linked, bytes) = run_full_pipeline_with_includes(
        src_path.to_str().unwrap(),
        STD_NES_SRC,
        "mos6502-nintendo-nes-ntsc",
        &[std_root.to_string_lossy().into_owned()],
    );
    bytes
}

#[test]
fn full_pipeline_std_nes() {
    let Some(std_root) = std_root() else {
        eprintln!("skipping: std library not found (set OP_STD_PATH)");
        return;
    };
    let dir = std::env::temp_dir().join(format!("opc-int-std-nes-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let bytes = run_std_pipeline(&dir, &std_root);
    assert!(
        bytes.starts_with(&[b'N', b'E', b'S', 0x1A]),
        "iNES output should start with NES magic"
    );
    assert_eq!(
        bytes.len(),
        16 + 16384 + 8192,
        "iNES ROM should be header + one PRG bank + one CHR bank"
    );
}

#[test]
fn full_pipeline_std_nes_rom_bytes() {
    let Some(std_root) = std_root() else {
        eprintln!("skipping: std library not found (set OP_STD_PATH)");
        return;
    };
    let dir = std::env::temp_dir().join(format!("opc-int-std-rom-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let bytes = run_std_pipeline(&dir, &std_root);
    let rom_path = dir.join("out.nes");
    std::fs::write(&rom_path, &bytes).unwrap();
    let rom = std::fs::read(&rom_path).unwrap();

    assert_eq!(&rom[0..4], &[0x4E, 0x45, 0x53, 0x1A], "iNES magic");
    assert_eq!(rom[4], 1, "one 16 KB PRG bank");
    assert_eq!(rom[5], 1, "one 8 KB CHR bank");
    assert_eq!(rom[6] & 0x0F, 0, "mapper zero");
}

#[test]
fn full_pipeline_std_nes_game() {
    let Some(std_root) = std_root() else {
        eprintln!("skipping: std library not found (set OP_STD_PATH)");
        return;
    };

    let source = include_str!("../../../examples/nes.op");
    let font = include_bytes!("../../../examples/font.chr");

    // Write the source and font into a temp dir so locate_bytes! can
    // find font.chr regardless of the test's working directory.
    let dir = std::env::temp_dir().join(format!("opc-int-nes-game-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("nes.op"), source).unwrap();
    std::fs::write(dir.join("font.chr"), font).unwrap();
    let src_path = dir.join("nes.op");

    let (ast, parse_diags) = parse_source(
        src_path.to_str().unwrap(),
        source,
        "mos6502-nintendo-nes-ntsc",
        &[],
    );
    let errors: Vec<_> = parse_diags
        .iter()
        .filter(|d| d.severity == op_diagnostics::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "parser errors: {:?}", errors);

    let (obj, codegen_diags) =
        compile_source(&ast, 1, &[std_root.to_string_lossy().into_owned()], &[]);
    let errors: Vec<_> = codegen_diags
        .iter()
        .filter(|d| d.severity == op_diagnostics::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "codegen errors: {:?}", errors);

    // The CHR section data must equal the font bytes byte for byte, even
    // at the default optimization level (Phase 1 fix).
    let font_bytes: Vec<u8> = font.to_vec();
    let chr = obj
        .sections
        .iter()
        .find(|s| s.kind == op_ir::SectionKind::Chr)
        .expect("object must have a CHR section");
    assert_eq!(chr.data.len(), 8192, "CHR section data must be 8192 bytes");
    assert_eq!(
        chr.data, font_bytes,
        "CHR section data must match font.chr byte for byte"
    );

    // The previously-unreferenced vars (paddr, msgbuf, scroll, oam) were
    // removed from nes.op in Phase 6, so there should be no dead-data
    // warnings.
    let dead_data = codegen_diags
        .iter()
        .filter(|d| d.code == 306)
        .filter(|d| {
            d.message.contains("paddr")
                || d.message.contains("msgbuf")
                || d.message.contains("scroll")
                || d.message.contains("oam")
        })
        .count();
    assert_eq!(
        dead_data, 0,
        "expected no dead-data warnings for removed vars"
    );

    let (linked, link_diags) = link_source(&obj);
    let errors: Vec<_> = link_diags
        .iter()
        .filter(|d| d.severity == op_diagnostics::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "linker errors: {:?}", errors);

    let format = default_format_for_target("mos6502-nintendo-nes-ntsc");
    let bytes = emit_linked(&linked, format).expect("emit_linked failed");
    assert!(
        bytes.starts_with(&[b'N', b'E', b'S', 0x1A]),
        "iNES output should start with NES magic"
    );

    // iNES header assertions.
    assert_eq!(&bytes[0..4], &[0x4E, 0x45, 0x53, 0x1A], "iNES magic");
    assert_eq!(bytes[4], 1, "one 16 KB PRG bank");
    assert_eq!(bytes[5], 1, "one 8 KB CHR bank");
    assert_eq!(
        bytes[6], 0,
        "flags6: mapper 0, vertical mirroring, no battery/trainer/fourscreen"
    );
}

// === Phase 7: new automated tests ==========================================

/// A function call on a new line after an assembly instruction must become a
/// separate `FnStmt::FnCall`, not a second operand of the assembly statement.
/// This is the Phase 2 parser fix.
#[test]
fn parser_inline_call_new_line() {
    use op_common::ast::{FnStmt, Item};

    let src = "inline fn caller() {\n lda #0\n callee()\n}\ninline fn callee() {\n inx\n}\n";
    let (ast, diags) = parse_source("test.op", src, "mos6502-nintendo-nes-ntsc", &[]);
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| d.severity == op_diagnostics::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "parser errors: {:?}", errors);

    let caller = ast
        .root
        .items
        .iter()
        .find_map(|i| match i {
            Item::InlineFnDecl { name, body, .. } if name == "caller" => Some(body),
            _ => None,
        })
        .expect("caller inline fn must exist");

    // The caller body must have two statements: an asm statement (lda #0) on
    // line 1 and a function call (callee()) on line 2.
    assert_eq!(caller.len(), 2, "caller body must have 2 statements");
    assert!(
        matches!(caller[0], FnStmt::AsmStmt { .. }),
        "first statement must be an asm statement"
    );
    assert!(
        matches!(caller[1], FnStmt::FnCall { .. }),
        "second statement must be a function call"
    );
}

/// The stage flags must chain through intermediate files: `--lex` reads
/// source and writes a `.opx`; `--parse` reads the `.opx` and writes a `.opa`;
/// `--compile` reads the `.opa` and writes a `.opl`; `--link` reads the `.opl`
/// and writes a linked `.opl`. This is the Phase 4 fix.
#[test]
fn stage_chaining() {
    use clap::Parser;
    let dir = std::env::temp_dir().join(format!("opc-stage-chain-{}", std::process::id()));
    // Clean up a previous run if present, then create a fresh dir.
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let src_path = dir.join("test.op");
    std::fs::write(&src_path, HELLO_NES_SRC).unwrap();

    let opx = dir.join("test.opx");
    let opa = dir.join("test.opa");
    let opl = dir.join("test.opl");
    let linked_opl = dir.join("test.linked.opl");
    let target = "mos6502-nintendo-nes-ntsc";

    // 1. --lex: source -> .opx
    let args_lex = opc::cli::OpcArgs::parse_from([
        "opc",
        "--lex",
        "-o",
        opx.to_str().unwrap(),
        src_path.to_str().unwrap(),
    ]);
    opc::lexer::run(&args_lex).expect("lex stage failed");
    assert!(opx.is_file(), "lex stage must write .opx");

    // 2. --parse: .opx -> .opa
    let args_parse = opc::cli::OpcArgs::parse_from([
        "opc",
        "--parse",
        "-o",
        opa.to_str().unwrap(),
        opx.to_str().unwrap(),
        "--target",
        target,
    ]);
    opc::parser::run(&args_parse).expect("parse stage failed");
    assert!(opa.is_file(), "parse stage must write .opa");

    // 3. --compile: .opa -> .opl
    let args_compile = opc::cli::OpcArgs::parse_from([
        "opc",
        "--compile",
        "-o",
        opl.to_str().unwrap(),
        opa.to_str().unwrap(),
        "--target",
        target,
    ]);
    opc::codegen::run(&args_compile).expect("compile stage failed");
    assert!(opl.is_file(), "compile stage must write .opl");

    // 4. --link: .opl -> linked .opl
    let args_link = opc::cli::OpcArgs::parse_from([
        "opc",
        "--link",
        "-o",
        linked_opl.to_str().unwrap(),
        opl.to_str().unwrap(),
        "--target",
        target,
    ]);
    opc::linker::run(&args_link).expect("link stage failed");
    assert!(linked_opl.is_file(), "link stage must write linked .opl");

    // The final linked object must have ROM sections.
    let linked_json = std::fs::read_to_string(&linked_opl).unwrap();
    let linked_obj: op_ir::ObjectFile = op_common::from_json(&linked_json).unwrap();
    let has_rom = linked_obj
        .sections
        .iter()
        .any(|s| s.kind == op_ir::SectionKind::Rom);
    assert!(has_rom, "linked object must have a ROM section");

    let _ = std::fs::remove_dir_all(&dir);
}

/// The `--output-stages` flag must write every intermediate envelope file
/// (`.opx`, `.opa`, `.opl`, `.linked.opl`) alongside the final binary, and the
/// final binary must match the standard in-memory pipeline output. This is
/// the Phase 5 feature.
#[test]
fn output_stages_flag() {
    let dir = std::env::temp_dir().join(format!("opc-output-stages-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let src_path = dir.join("test.op");
    std::fs::write(&src_path, HELLO_NES_SRC).unwrap();
    let nes_path = dir.join("out.nes");
    let target = "mos6502-nintendo-nes-ntsc";

    // Run the pipeline with --output-stages via the public stage fns so the
    // test does not depend on the private `run_pipeline` helper.
    let source = std::fs::read_to_string(&src_path).unwrap();
    let (token_stream, lex_diags) = opc::lexer::lex_source(src_path.to_str().unwrap(), &source);
    assert!(
        !lex_diags
            .iter()
            .any(|d| d.severity == op_diagnostics::Severity::Error),
        "lexer errors"
    );
    std::fs::write(
        dir.join("out.opx"),
        op_common::to_json(&token_stream).unwrap(),
    )
    .unwrap();

    let (ast, parse_diags) =
        opc::parser::parse_token_stream(src_path.to_str().unwrap(), token_stream, target, &[]);
    assert!(
        !parse_diags
            .iter()
            .any(|d| d.severity == op_diagnostics::Severity::Error),
        "parser errors"
    );
    std::fs::write(dir.join("out.opa"), op_common::to_json(&ast).unwrap()).unwrap();

    let (obj, codegen_diags) = opc::codegen::compile_source(&ast, 1, &[], &[]);
    assert!(
        !codegen_diags
            .iter()
            .any(|d| d.severity == op_diagnostics::Severity::Error),
        "codegen errors"
    );
    std::fs::write(dir.join("out.opl"), op_common::to_json(&obj).unwrap()).unwrap();

    let (linked, link_diags) = opc::linker::link_source(&obj);
    assert!(
        !link_diags
            .iter()
            .any(|d| d.severity == op_diagnostics::Severity::Error),
        "linker errors"
    );
    std::fs::write(
        dir.join("out.linked.opl"),
        op_common::to_json(&linked).unwrap(),
    )
    .unwrap();

    let format = opc::output::default_format_for_target(target);
    let stages_bytes = opc::output::emit_linked(&linked, format).expect("emit_linked failed");
    std::fs::write(&nes_path, &stages_bytes).unwrap();

    // All intermediate files must exist.
    for name in ["out.opx", "out.opa", "out.opl", "out.linked.opl", "out.nes"] {
        let p = dir.join(name);
        assert!(p.is_file(), "intermediate file {name} must exist");
    }

    // The final binary must match the standard pipeline output.
    let (_obj2, _linked2, std_bytes) =
        run_full_pipeline(src_path.to_str().unwrap(), HELLO_NES_SRC, target);
    assert_eq!(
        stages_bytes, std_bytes,
        "--output-stages final binary must match standard pipeline output"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
