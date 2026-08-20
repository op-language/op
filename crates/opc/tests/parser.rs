//! Parser unit tests.
//!
//! These tests call `opc::parser::parse_source()` on source strings and
//! assert the AST structure.

use op_common::ast::{Expr, FnStmt, Item, Operand, Type};
use opc::parser::parse_source;

/// Helper: parse a source string with a default target and return the
/// root module's items.
fn parse_items(src: &str) -> Vec<Item> {
    let (ast, _diags) = parse_source("test.op", src, "mos6502-nintendo-nes-ntsc", &[]);
    ast.root.items
}

/// Helper: parse a source string and return the single item.
fn parse_one(src: &str) -> Item {
    let items = parse_items(src);
    assert_eq!(items.len(), 1, "expected 1 item, got {}", items.len());
    items.into_iter().next().unwrap()
}

// === Module items: const ===================================================

#[test]
fn parse_const_decl() {
    let item = parse_one("const SCREEN_WIDTH: u8 = 256;");
    match item {
        Item::ConstDecl {
            name,
            ty,
            value,
            evaluated_value,
            ..
        } => {
            assert_eq!(name, "SCREEN_WIDTH");
            assert!(matches!(ty, Type::Named { ref name } if name == "u8"));
            assert!(matches!(value, Expr::Number { value: 256 }));
            assert_eq!(evaluated_value, Some(256));
        }
        _ => panic!("expected ConstDecl, got {:?}", item),
    }
}

#[test]
fn parse_const_with_binop() {
    let item = parse_one("const MASK: u8 = 0x0F | 0x10;");
    match item {
        Item::ConstDecl {
            evaluated_value, ..
        } => {
            assert_eq!(evaluated_value, Some(0x1F));
        }
        _ => panic!("expected ConstDecl"),
    }
}

#[test]
fn parse_const_with_macro() {
    let item = parse_one("const LO: u8 = lo!(0x1234);");
    match item {
        Item::ConstDecl {
            evaluated_value, ..
        } => {
            assert_eq!(evaluated_value, Some(0x34));
        }
        _ => panic!("expected ConstDecl"),
    }
}

// === Module items: var =====================================================

#[test]
fn parse_var_decl() {
    let item = parse_one("counter: u8;");
    match item {
        Item::VarDecl {
            name,
            is_volatile,
            ty,
            ..
        } => {
            assert_eq!(name, "counter");
            assert!(!is_volatile);
            assert!(matches!(ty, Type::Named { ref name } if name == "u8"));
        }
        _ => panic!("expected VarDecl"),
    }
}

#[test]
fn parse_volatile_var_decl() {
    let item = parse_one("volatile flag: u8 = 0;");
    match item {
        Item::VarDecl { is_volatile, .. } => {
            assert!(is_volatile);
        }
        _ => panic!("expected VarDecl"),
    }
}

#[test]
fn parse_var_with_init() {
    let item = parse_one("palcol: u8 = 0;");
    match item {
        Item::VarDecl { init, .. } => {
            assert!(init.is_some());
        }
        _ => panic!("expected VarDecl"),
    }
}

#[test]
fn parse_var_array() {
    let item = parse_one("msgbuf: [u8; 64];");
    match item {
        Item::VarDecl { ty, array_dim, .. } => {
            // [u8; 64] is an array type, not a named type with array_dim.
            assert!(matches!(ty, Type::Array { .. }));
            assert!(array_dim.is_none());
        }
        _ => panic!("expected VarDecl"),
    }
}

// === Module items: fn ======================================================

#[test]
fn parse_fn_decl() {
    let item = parse_one("fn main() { lda 0 }");
    match item {
        Item::FnDecl {
            name,
            is_noreturn,
            body,
            ..
        } => {
            assert_eq!(name, "main");
            assert!(!is_noreturn);
            assert_eq!(body.len(), 1);
        }
        _ => panic!("expected FnDecl"),
    }
}

#[test]
fn parse_noreturn_fn_decl() {
    let item = parse_one("noreturn fn main() { loop { } }");
    match item {
        Item::FnDecl { is_noreturn, .. } => {
            assert!(is_noreturn);
        }
        _ => panic!("expected FnDecl"),
    }
}

#[test]
fn parse_fn_with_attribute() {
    let item = parse_one("#[interrupt(reset)] fn main() { lda 0 }");
    match item {
        Item::FnDecl { attributes, .. } => {
            assert_eq!(attributes.len(), 1);
            assert_eq!(attributes[0].path, "interrupt");
        }
        _ => panic!("expected FnDecl"),
    }
}

// === Module items: inline fn ===============================================

#[test]
fn parse_inline_fn_decl() {
    let item = parse_one("inline fn assign(dest, value) { lda value }");
    match item {
        Item::InlineFnDecl {
            name, params, body, ..
        } => {
            assert_eq!(name, "assign");
            assert_eq!(params.len(), 2);
            assert_eq!(params[0], "dest");
            assert_eq!(params[1], "value");
            assert_eq!(body.len(), 1);
        }
        _ => panic!("expected InlineFnDecl"),
    }
}

#[test]
fn parse_inline_fn_no_params() {
    let item = parse_one("inline fn reset_stack() { ldx 0xFF }");
    match item {
        Item::InlineFnDecl { params, .. } => {
            assert!(params.is_empty());
        }
        _ => panic!("expected InlineFnDecl"),
    }
}

// === Module items: struct ==================================================

#[test]
fn parse_struct_decl() {
    let item = parse_one("struct POINT { x: u16, y: u16 }");
    match item {
        Item::StructDecl { name, fields, .. } => {
            assert_eq!(name, "POINT");
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].name, "x");
            assert_eq!(fields[1].name, "y");
        }
        _ => panic!("expected StructDecl"),
    }
}

#[test]
fn parse_struct_with_array_field() {
    let item = parse_one("struct PLAYER { name: [u8; 16] }");
    match item {
        Item::StructDecl { fields, .. } => {
            // [u8; 16] is an array type.
            assert!(matches!(fields[0].ty, Type::Array { .. }));
        }
        _ => panic!("expected StructDecl"),
    }
}

// === Module items: type ====================================================

#[test]
fn parse_type_decl() {
    let item = parse_one("type COORDS = POINT;");
    match item {
        Item::TypeDecl { name, ty, .. } => {
            assert_eq!(name, "COORDS");
            assert!(matches!(ty, Type::Named { ref name } if name == "POINT"));
        }
        _ => panic!("expected TypeDecl"),
    }
}

// === Module items: enum ====================================================

#[test]
fn parse_enum_decl() {
    let item = parse_one("enum PPU { CNT0 = 0x2000, STATUS = 0x2002 }");
    match item {
        Item::EnumDecl { name, variants, .. } => {
            assert_eq!(name, "PPU");
            assert_eq!(variants.len(), 2);
            assert_eq!(variants[0].name, "CNT0");
            assert!(variants[0].value.is_some());
            assert_eq!(variants[1].name, "STATUS");
        }
        _ => panic!("expected EnumDecl"),
    }
}

#[test]
fn parse_enum_without_values() {
    let item = parse_one("enum INTERRUPT { reset, nmi, irq }");
    match item {
        Item::EnumDecl { variants, .. } => {
            assert_eq!(variants.len(), 3);
            assert!(variants[0].value.is_none());
        }
        _ => panic!("expected EnumDecl"),
    }
}

// === Module items: mod =====================================================

#[test]
fn parse_mod_decl_file() {
    let item = parse_one("mod graphics;");
    match item {
        Item::ModDecl {
            name,
            is_pub,
            body,
            resolved,
            ..
        } => {
            assert_eq!(name, "graphics");
            assert!(!is_pub);
            assert!(body.is_none());
            assert!(resolved.is_none()); // file not found
        }
        _ => panic!("expected ModDecl"),
    }
}

#[test]
fn parse_pub_mod_decl() {
    let item = parse_one("pub mod graphics;");
    match item {
        Item::ModDecl { is_pub, .. } => {
            assert!(is_pub);
        }
        _ => panic!("expected ModDecl"),
    }
}

#[test]
fn parse_mod_decl_inline() {
    let item = parse_one("mod audio { fn play() { } }");
    match item {
        Item::ModDecl {
            name,
            body,
            resolved,
            ..
        } => {
            assert_eq!(name, "audio");
            assert!(body.is_some());
            assert!(resolved.is_none());
            let items = body.unwrap();
            assert_eq!(items.len(), 1);
        }
        _ => panic!("expected ModDecl"),
    }
}

// === Module items: use =====================================================

#[test]
fn parse_use_decl() {
    let item = parse_one("use std::cpu::*;");
    match item {
        Item::UseDecl { is_pub, trees, .. } => {
            assert!(!is_pub);
            assert_eq!(trees.len(), 1);
        }
        _ => panic!("expected UseDecl"),
    }
}

#[test]
fn parse_pub_use_decl() {
    let item = parse_one("pub use mos6502 as cpu;");
    match item {
        Item::UseDecl { is_pub, .. } => {
            assert!(is_pub);
        }
        _ => panic!("expected UseDecl"),
    }
}

#[test]
fn parse_use_group() {
    let item = parse_one("use std::{cpu, machine};");
    match item {
        Item::UseDecl { trees, .. } => {
            assert_eq!(trees.len(), 1);
        }
        _ => panic!("expected UseDecl"),
    }
}

// === Function body: assembly statements ====================================

#[test]
fn parse_asm_stmt() {
    let item = parse_one("fn test() { lda 0x2000 }");
    match item {
        Item::FnDecl { body, .. } => {
            assert_eq!(body.len(), 1);
            match &body[0] {
                FnStmt::AsmStmt { opcode, operands } => {
                    assert_eq!(opcode, "lda");
                    assert_eq!(operands.len(), 1);
                }
                _ => panic!("expected AsmStmt"),
            }
        }
        _ => panic!("expected FnDecl"),
    }
}

#[test]
fn parse_asm_no_operands() {
    let item = parse_one("fn test() { inx }");
    match item {
        Item::FnDecl { body, .. } => match &body[0] {
            FnStmt::AsmStmt { opcode, operands } => {
                assert_eq!(opcode, "inx");
                assert!(operands.is_empty());
            }
            _ => panic!("expected AsmStmt"),
        },
        _ => panic!("expected FnDecl"),
    }
}

#[test]
fn parse_asm_immediate() {
    let item = parse_one("fn test() { lda #0 }");
    match item {
        Item::FnDecl { body, .. } => match &body[0] {
            FnStmt::AsmStmt { operands, .. } => {
                assert_eq!(operands.len(), 1);
                assert!(matches!(&operands[0], Operand::Immediate { .. }));
            }
            _ => panic!("expected AsmStmt"),
        },
        _ => panic!("expected FnDecl"),
    }
}

#[test]
fn parse_asm_selector() {
    let item = parse_one("fn test() { sta PPU::CNT0 }");
    match item {
        Item::FnDecl { body, .. } => match &body[0] {
            FnStmt::AsmStmt { operands, .. } => {
                assert_eq!(operands.len(), 1);
                match &operands[0] {
                    Operand::MemoryOperand { expr, .. } => match expr {
                        Expr::Selector { path, accesses } => {
                            assert_eq!(path[0], "PPU");
                            assert_eq!(accesses.len(), 1);
                        }
                        _ => panic!("expected Selector expr in MemoryOperand"),
                    },
                    _ => panic!("expected MemoryOperand with selector expr"),
                }
            }
            _ => panic!("expected AsmStmt"),
        },
        _ => panic!("expected FnDecl"),
    }
}

#[test]
fn parse_asm_register_ref() {
    let item = parse_one("fn test() { lda cpu::x }");
    match item {
        Item::FnDecl { body, .. } => match &body[0] {
            FnStmt::AsmStmt { operands, .. } => {
                assert!(matches!(&operands[0], Operand::RegisterRef { ref name } if name == "x"));
            }
            _ => panic!("expected AsmStmt"),
        },
        _ => panic!("expected FnDecl"),
    }
}

#[test]
fn parse_asm_label_ref() {
    let item = parse_one("fn test() { jmp 'loop }");
    match item {
        Item::FnDecl { body, .. } => match &body[0] {
            FnStmt::AsmStmt { operands, .. } => {
                assert!(matches!(&operands[0], Operand::LabelRef { ref name } if name == "loop"));
            }
            _ => panic!("expected AsmStmt"),
        },
        _ => panic!("expected FnDecl"),
    }
}

#[test]
fn parse_asm_stmt_stops_at_line_break() {
    // An assembly instruction on one line must not consume a function call
    // on the next line as a second operand. The function call must become a
    // separate FnStmt::FnCall statement. This is the regression test for the
    // wait_for_func bug: before the fix, the parser folded wait_for_func()
    // into ldx's operands because the lexer discards newlines.
    let src = "inline fn wait_for(amount) { ldx amount\nwait_for_func() }\ninline fn wait_for_func() { dex }";
    let (ast, _diags) = parse_source("test.op", src, "mos6502-nintendo-nes-ntsc", &[]);
    // Find the wait_for inline fn.
    let inline = ast
        .root
        .items
        .iter()
        .find_map(|i| match i {
            Item::InlineFnDecl { name, body, .. } if name == "wait_for" => Some(body),
            _ => None,
        })
        .expect("wait_for inline fn must exist");
    // The body must have two statements, not one.
    assert_eq!(
        inline.len(),
        2,
        "expected 2 statements in wait_for body, got {}: {:?}",
        inline.len(),
        inline,
    );
    // First statement: AsmStmt with opcode "ldx" and one operand.
    assert!(
        matches!(&inline[0], FnStmt::AsmStmt { opcode, operands } if opcode == "ldx" && operands.len() == 1),
        "first statement must be AsmStmt ldx with 1 operand, got {:?}",
        inline[0],
    );
    // Second statement: FnCall to wait_for_func.
    assert!(
        matches!(&inline[1], FnStmt::FnCall { name, args } if name == "wait_for_func" && args.is_empty()),
        "second statement must be FnCall wait_for_func, got {:?}",
        inline[1],
    );
}

#[test]
fn parse_asm_stmt_same_line_operands() {
    // Multiple operands on the same line must still be consumed by the
    // assembly instruction. This confirms the line-break fix does not break
    // multi-operand instructions on one line.
    let item = parse_one("fn test() { sta 0x2000 0x1234 }");
    match item {
        Item::FnDecl { body, .. } => match &body[0] {
            FnStmt::AsmStmt { opcode, operands } => {
                assert_eq!(opcode, "sta");
                assert_eq!(operands.len(), 2, "same-line operands must be consumed");
            }
            _ => panic!("expected AsmStmt"),
        },
        _ => panic!("expected FnDecl"),
    }
}

// === Function body: control flow ===========================================

#[test]
fn parse_if_stmt() {
    let item = parse_one("fn test() { if (set) { lda 0 } }");
    match item {
        Item::FnDecl { body, .. } => {
            assert!(matches!(&body[0], FnStmt::IfStmt { .. }));
        }
        _ => panic!("expected FnDecl"),
    }
}

#[test]
fn parse_if_else_stmt() {
    let item = parse_one("fn test() { if (set) { lda 0 } else { lda 1 } }");
    match item {
        Item::FnDecl { body, .. } => match &body[0] {
            FnStmt::IfStmt { else_block, .. } => {
                assert!(else_block.is_some());
            }
            _ => panic!("expected IfStmt"),
        },
        _ => panic!("expected FnDecl"),
    }
}

#[test]
fn parse_while_stmt() {
    let item = parse_one("fn test() { while (not zero) { dex } }");
    match item {
        Item::FnDecl { body, .. } => {
            assert!(matches!(&body[0], FnStmt::WhileStmt { .. }));
        }
        _ => panic!("expected FnDecl"),
    }
}

#[test]
fn parse_do_while_stmt() {
    let item = parse_one("fn test() { do { lda 0 } while (set) }");
    match item {
        Item::FnDecl { body, .. } => {
            assert!(matches!(&body[0], FnStmt::DoWhileStmt { .. }));
        }
        _ => panic!("expected FnDecl"),
    }
}

#[test]
fn parse_loop_stmt() {
    let item = parse_one("fn test() { loop { lda 0 } }");
    match item {
        Item::FnDecl { body, .. } => {
            assert!(matches!(&body[0], FnStmt::LoopStmt { .. }));
        }
        _ => panic!("expected FnDecl"),
    }
}

#[test]
fn parse_switch_stmt() {
    let item = parse_one("fn test() { switch (cpu::x) { case 12 { lda 0 } default { lda 1 } } }");
    match item {
        Item::FnDecl { body, .. } => match &body[0] {
            FnStmt::SwitchStmt { register, cases } => {
                assert_eq!(register, "x");
                assert_eq!(cases.len(), 2);
            }
            _ => panic!("expected SwitchStmt"),
        },
        _ => panic!("expected FnDecl"),
    }
}

#[test]
fn parse_return_stmt() {
    let item = parse_one("fn test() { return }");
    match item {
        Item::FnDecl { body, .. } => {
            assert!(matches!(&body[0], FnStmt::ReturnStmt));
        }
        _ => panic!("expected FnDecl"),
    }
}

#[test]
fn parse_fn_call_stmt() {
    let item = parse_one("fn test() { foo() }");
    match item {
        Item::FnDecl { body, .. } => match &body[0] {
            FnStmt::FnCall { name, args } => {
                assert_eq!(name, "foo");
                assert!(args.is_empty());
            }
            _ => panic!("expected FnCall"),
        },
        _ => panic!("expected FnDecl"),
    }
}

#[test]
fn parse_fn_call_with_args() {
    let item = parse_one("fn test() { assign(PPU::CNT0, 0) }");
    match item {
        Item::FnDecl { body, .. } => match &body[0] {
            FnStmt::FnCall { name, args } => {
                assert_eq!(name, "assign");
                assert_eq!(args.len(), 2);
            }
            _ => panic!("expected FnCall"),
        },
        _ => panic!("expected FnDecl"),
    }
}

#[test]
fn parse_label() {
    let item = parse_one("fn test() { 'loop: lda 0 }");
    match item {
        Item::FnDecl { body, .. } => match &body[0] {
            FnStmt::Label { name, .. } => {
                assert_eq!(name, "loop");
            }
            _ => panic!("expected Label"),
        },
        _ => panic!("expected FnDecl"),
    }
}

// === Expressions ===========================================================

#[test]
fn parse_expr_binary() {
    let item = parse_one("const X: u8 = 1 + 2 * 3;");
    match item {
        Item::ConstDecl {
            evaluated_value, ..
        } => {
            assert_eq!(evaluated_value, Some(7));
        }
        _ => panic!("expected ConstDecl"),
    }
}

#[test]
fn parse_expr_parens() {
    let item = parse_one("const X: u8 = (1 + 2) * 3;");
    match item {
        Item::ConstDecl {
            evaluated_value, ..
        } => {
            assert_eq!(evaluated_value, Some(9));
        }
        _ => panic!("expected ConstDecl"),
    }
}

#[test]
fn parse_expr_unary() {
    let item = parse_one("const X: u8 = ~0xFF;");
    match item {
        Item::ConstDecl {
            evaluated_value, ..
        } => {
            assert_eq!(evaluated_value, Some(-256));
        }
        _ => panic!("expected ConstDecl"),
    }
}

#[test]
fn parse_expr_shift() {
    let item = parse_one("const X: u8 = 1 << 4;");
    match item {
        Item::ConstDecl {
            evaluated_value, ..
        } => {
            assert_eq!(evaluated_value, Some(16));
        }
        _ => panic!("expected ConstDecl"),
    }
}

#[test]
fn parse_expr_all_operators() {
    let item = parse_one("const X: u8 = 0xFF & 0x0F | 0xF0 ^ 0x10;");
    match item {
        Item::ConstDecl {
            evaluated_value, ..
        } => {
            // 0xFF & 0x0F = 0x0F
            // 0x0F | 0xF0 = 0xFF
            // 0xFF ^ 0x10 = 0xEF
            assert_eq!(evaluated_value, Some(0xEF));
        }
        _ => panic!("expected ConstDecl"),
    }
}

// === Attributes ============================================================

#[test]
fn parse_cfg_attribute() {
    let items = parse_items("#[cfg(cpu = \"mos6502\")] const X: u8 = 0;");
    // With target mos6502-nintendo-nes-ntsc, cpu matches.
    assert_eq!(items.len(), 1);
}

#[test]
fn parse_cfg_attribute_dropped() {
    let items = parse_items("#[cfg(cpu = \"z80\")] const X: u8 = 0;");
    // With target mos6502-nintendo-nes-ntsc, cpu does not match.
    assert_eq!(items.len(), 0);
}

#[test]
fn parse_rom_block_attribute() {
    let items = parse_items("#[rom(org = 0xC000, bank = 0)] { const X: u8 = 0 }");
    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::BlockAttribute { attr, items } => {
            assert_eq!(attr.path, "rom");
            assert_eq!(items.len(), 1);
        }
        _ => panic!("expected BlockAttribute"),
    }
}

#[test]
fn parse_ram_block_attribute() {
    let items = parse_items("#[ram(org = 0x0000, maxsize = 0x100)] { counter: u8 }");
    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::BlockAttribute { attr, items } => {
            assert_eq!(attr.path, "ram");
            assert_eq!(items.len(), 1);
        }
        _ => panic!("expected BlockAttribute"),
    }
}

#[test]
fn parse_standalone_attribute() {
    let items = parse_items("#[setpad(0xFF)]");
    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::BlockAttribute { attr, items } => {
            assert_eq!(attr.path, "setpad");
            assert!(items.is_empty());
        }
        _ => panic!("expected BlockAttribute"),
    }
}

// === Placement macros ======================================================

#[test]
fn parse_placement_locate_fn() {
    let items = parse_items("locate_fn!(nes_code::main);");
    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::Placement {
            macro_name,
            argument,
            ..
        } => {
            assert_eq!(macro_name, "locate_fn");
            match argument {
                op_common::ast::PlacementArg::Path { segments } => {
                    assert_eq!(segments, &["nes_code", "main"]);
                }
                _ => panic!("expected Path argument"),
            }
        }
        _ => panic!("expected Placement"),
    }
}

#[test]
fn parse_placement_locate_bytes() {
    let items = parse_items("locate_bytes!(\"font.chr\")");
    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::Placement {
            macro_name,
            argument,
            ..
        } => {
            assert_eq!(macro_name, "locate_bytes");
            match argument {
                op_common::ast::PlacementArg::String_ { value } => {
                    assert_eq!(value, "\"font.chr\"");
                }
                _ => panic!("expected String argument"),
            }
        }
        _ => panic!("expected Placement"),
    }
}

// === Multiple items ========================================================

#[test]
fn parse_multiple_items() {
    let src = "const A: u8 = 1; const B: u8 = 2; fn foo() { }";
    let items = parse_items(src);
    assert_eq!(items.len(), 3);
}

#[test]
fn parse_empty_source() {
    let items = parse_items("");
    assert!(items.is_empty());
}

#[test]
fn parse_comments_only() {
    let items = parse_items("// just a comment\n/* block */");
    assert!(items.is_empty());
}

// === Full source ===========================================================

#[test]
fn parse_full_fn() {
    let src = "fn main() { lda 0 sta PPU::CNT0 if (set) { lda 1 } else { lda 2 } }";
    let item = parse_one(src);
    match item {
        Item::FnDecl { body, .. } => {
            assert_eq!(body.len(), 3);
            assert!(matches!(&body[0], FnStmt::AsmStmt { .. }));
            assert!(matches!(&body[1], FnStmt::AsmStmt { .. }));
            assert!(matches!(&body[2], FnStmt::IfStmt { .. }));
        }
        _ => panic!("expected FnDecl"),
    }
}

#[test]
fn parse_do_while_with_modifier() {
    let src = "fn test() { do { lda PPU::STATUS } while (is plus) }";
    let item = parse_one(src);
    match item {
        Item::FnDecl { body, .. } => match &body[0] {
            FnStmt::DoWhileStmt { condition, .. } => {
                assert!(condition.modifiers.contains(&"is".to_string()));
                assert_eq!(condition.keyword, "plus");
            }
            _ => panic!("expected DoWhileStmt"),
        },
        _ => panic!("expected FnDecl"),
    }
}
