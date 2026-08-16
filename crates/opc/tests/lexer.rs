//! Comprehensive lexer unit tests.
//!
//! These tests call `opc::lexer::lex_source()` on source strings and
//! assert the exact `TokenStream` (token types, values, lines, columns).

use op_common::TokenStream;
use opc::lexer::lex_source;

/// Helper: lex a source string and return the token stream (discards
/// diagnostics for tests that do not check them).
fn lex(src: &str) -> TokenStream {
    let (stream, _diags) = lex_source("test.op", src);
    stream
}

/// Helper: assert a single-token stream has the expected type and value.
fn assert_single(stream: &TokenStream, expected_type: &str, expected_value: &str) {
    assert_eq!(
        stream.tokens.len(),
        1,
        "expected 1 token, got {}",
        stream.tokens.len()
    );
    assert_eq!(stream.tokens[0].kind, expected_type);
    assert_eq!(stream.tokens[0].value, expected_value);
}

/// Helper: assert a token at a given index has the expected type and value.
fn assert_tok(stream: &TokenStream, idx: usize, expected_type: &str, expected_value: &str) {
    assert!(
        idx < stream.tokens.len(),
        "expected token at index {}, but only {} tokens",
        idx,
        stream.tokens.len()
    );
    assert_eq!(stream.tokens[idx].kind, expected_type, "token {} type", idx);
    assert_eq!(
        stream.tokens[idx].value, expected_value,
        "token {} value",
        idx
    );
}

// === Keywords ===============================================================

#[test]
fn lex_keywords() {
    let cases = &[
        ("fn", "Kw_fn"),
        ("inline", "Kw_inline"),
        ("noreturn", "Kw_noreturn"),
        ("return", "Kw_return"),
        ("volatile", "Kw_volatile"),
        ("struct", "Kw_struct"),
        ("type", "Kw_type"),
        ("enum", "Kw_enum"),
        ("const", "Kw_const"),
        ("mod", "Kw_mod"),
        ("use", "Kw_use"),
        ("pub", "Kw_pub"),
        ("if", "Kw_if"),
        ("else", "Kw_else"),
        ("while", "Kw_while"),
        ("do", "Kw_do"),
        ("loop", "Kw_loop"),
        ("switch", "Kw_switch"),
        ("case", "Kw_case"),
        ("default", "Kw_default"),
        ("near", "Kw_near"),
        ("far", "Kw_far"),
        ("as", "Kw_as"),
        ("lib", "Kw_lib"),
        ("self", "Kw_self"),
        ("super", "Kw_super"),
        ("true", "Kw_true"),
        ("false", "Kw_false"),
    ];
    for (src, expected) in cases {
        let stream = lex(src);
        assert_single(&stream, expected, src);
    }
}

// === Primitive types ========================================================

#[test]
fn lex_primitive_types() {
    let cases = &[
        ("u8", "Type_u8"),
        ("i8", "Type_i8"),
        ("u16", "Type_u16"),
        ("i16", "Type_i16"),
        ("u32", "Type_u32"),
        ("i32", "Type_i32"),
        ("bool", "Type_bool"),
        ("pointer", "Type_pointer"),
    ];
    for (src, expected) in cases {
        let stream = lex(src);
        assert_single(&stream, expected, src);
    }
}

// === Operators and punctuation ==============================================

#[test]
fn lex_multi_char_operators() {
    let cases = &[
        ("::", "Op_colon_colon"),
        (">>", "Op_shr"),
        ("<<", "Op_shl"),
        (">=", "Op_ge"),
        ("<=", "Op_le"),
        ("==", "Op_eq"),
        ("!=", "Op_ne"),
    ];
    for (src, expected) in cases {
        let stream = lex(src);
        assert_single(&stream, expected, src);
    }
}

#[test]
fn lex_single_char_operators() {
    let cases: &[(&str, &str)] = &[
        ("#", "Op_hash"),
        (".", "Op_dot"),
        ("+", "Op_plus"),
        ("-", "Op_minus"),
        ("*", "Op_star"),
        ("/", "Op_slash"),
        ("%", "Op_percent"),
        ("~", "Op_tilde"),
        ("!", "Op_bang"),
        ("&", "Op_amp"),
        ("^", "Op_caret"),
        ("|", "Op_pipe"),
        (">", "Op_gt"),
        ("<", "Op_lt"),
        ("=", "Op_assign"),
        ("(", "Op_lparen"),
        (")", "Op_rparen"),
        ("{", "Op_lbrace"),
        ("}", "Op_rbrace"),
        ("[", "Op_lbracket"),
        ("]", "Op_rbracket"),
        (":", "Op_colon"),
        (",", "Op_comma"),
        (";", "Op_semicolon"),
    ];
    for (src, expected) in cases {
        let stream = lex(src);
        assert_single(&stream, expected, src);
    }
}

#[test]
fn lex_operators_not_split() {
    // :: should be one token, not two :
    let stream = lex("::");
    assert_eq!(stream.tokens.len(), 1);
    assert_eq!(stream.tokens[0].kind, "Op_colon_colon");

    // >= should be one token, not > and =
    let stream = lex(">=");
    assert_eq!(stream.tokens.len(), 1);
    assert_eq!(stream.tokens[0].kind, "Op_ge");

    // != should be one token, not ! and =
    let stream = lex("!=");
    assert_eq!(stream.tokens.len(), 1);
    assert_eq!(stream.tokens[0].kind, "Op_ne");
}

// === Number literals ========================================================

#[test]
fn lex_decimal_numbers() {
    let stream = lex("123");
    assert_single(&stream, "NUMBER", "123");

    let stream = lex("0");
    assert_single(&stream, "NUMBER", "0");
}

#[test]
fn lex_binary_numbers() {
    let stream = lex("%11110000");
    assert_single(&stream, "NUMBER", "%11110000");

    let stream = lex("%0");
    assert_single(&stream, "NUMBER", "%0");
}

#[test]
fn lex_hex_numbers() {
    let stream = lex("0xFF");
    assert_single(&stream, "NUMBER", "0xFF");

    let stream = lex("0x2000");
    assert_single(&stream, "NUMBER", "0x2000");

    let stream = lex("0xdeadbeef");
    assert_single(&stream, "NUMBER", "0xdeadbeef");
}

// === String literals =======================================================

#[test]
fn lex_string_literal() {
    let stream = lex("\"Hello, NES!\"");
    assert_single(&stream, "STRING", "\"Hello, NES!\"");
}

#[test]
fn lex_string_with_escapes() {
    let stream = lex("\"\\n\\r\\t\\0\\a\\\\\\\"\"");
    assert_single(&stream, "STRING", "\"\\n\\r\\t\\0\\a\\\\\\\"\"");
}

#[test]
fn lex_invalid_escape_produces_diagnostic() {
    let (_stream, diags) = lex_source("test.op", "\"\\q\"");
    assert!(
        diags.iter().any(|d| d.code == 102),
        "expected error code 102"
    );
}

#[test]
fn lex_unterminated_string_produces_diagnostic() {
    let (_stream, diags) = lex_source("test.op", "\"unterminated");
    assert!(
        diags.iter().any(|d| d.code == 103),
        "expected error code 103"
    );
}

// === Labels =================================================================

#[test]
fn lex_label_def() {
    let stream = lex("'loop:");
    assert_single(&stream, "LABEL_DEF", "'loop:");
}

#[test]
fn lex_label_ref() {
    let stream = lex("'loop");
    assert_single(&stream, "LABEL_REF", "'loop");
}

#[test]
fn lex_label_in_context() {
    let stream = lex("'loop: lda 0x0200");
    assert_eq!(stream.tokens.len(), 3);
    assert_tok(&stream, 0, "LABEL_DEF", "'loop:");
    assert_tok(&stream, 1, "OPCODE", "lda");
    assert_tok(&stream, 2, "NUMBER", "0x0200");
}

// === Identifiers ============================================================

#[test]
fn lex_identifiers() {
    let stream = lex("foo");
    assert_single(&stream, "IDENT", "foo");

    let stream = lex("_bar");
    assert_single(&stream, "IDENT", "_bar");

    let stream = lex("foo_bar_123");
    assert_single(&stream, "IDENT", "foo_bar_123");
}

// === Opcodes ================================================================

#[test]
fn lex_6502_opcodes() {
    for src in &["lda", "sta", "inx", "jmp", "jsr", "rts"] {
        let stream = lex(src);
        assert_single(&stream, "OPCODE", src);
    }
}

#[test]
fn lex_z80_opcodes() {
    for src in &["ld", "push", "pop", "jp", "call", "ret"] {
        let stream = lex(src);
        assert_single(&stream, "OPCODE", src);
    }
}

#[test]
fn lex_68000_opcodes() {
    for src in &["move", "addq", "lea", "tst", "bsr"] {
        let stream = lex(src);
        assert_single(&stream, "OPCODE", src);
    }
}

#[test]
fn lex_opcode_case_insensitive() {
    for src in &["LDA", "Lda", "lDa"] {
        let stream = lex(src);
        assert_single(&stream, "OPCODE", src);
    }
}

// === Condition keywords =====================================================

#[test]
fn lex_condition_keywords() {
    let cases = &[
        ("plus", "Cond_plus"),
        ("carry", "Cond_carry"),
        ("zero", "Cond_zero"),
        ("not_zero", "Cond_not_zero"),
        ("greater_or_equal", "Cond_greater_or_equal"),
    ];
    for (src, expected) in cases {
        let stream = lex(src);
        assert_single(&stream, expected, src);
    }
}

// === Condition modifiers ====================================================

#[test]
fn lex_condition_modifiers() {
    let cases = &[
        ("is", "Mod_is"),
        ("has", "Mod_has"),
        ("no", "Mod_no"),
        ("not", "Mod_not"),
    ];
    for (src, expected) in cases {
        let stream = lex(src);
        assert_single(&stream, expected, src);
    }
}

// === Mode prefixes ==========================================================

#[test]
fn lex_mode_prefixes() {
    let cases = &[
        ("zp", "Mode_zp"),
        ("abs", "Mode_abs"),
        ("rel", "Mode_rel"),
        ("ind", "Mode_ind"),
        ("idx", "Mode_idx"),
        ("ind_l", "Mode_ind_l"),
        ("ind_idx", "Mode_ind_idx"),
    ];
    for (src, expected) in cases {
        let stream = lex(src);
        assert_single(&stream, expected, src);
    }
}

// === Compile-time macros ====================================================

#[test]
fn lex_compile_macros() {
    let stream = lex("lo!(x)");
    assert_tok(&stream, 0, "Macro_lo", "lo");
    assert_tok(&stream, 1, "Op_lparen", "(");
    assert_tok(&stream, 2, "IDENT", "x");
    assert_tok(&stream, 3, "Op_rparen", ")");

    let stream = lex("hi!(value)");
    assert_tok(&stream, 0, "Macro_hi", "hi");

    let stream = lex("sizeof!(main)");
    assert_tok(&stream, 0, "Macro_sizeof", "sizeof");
}

// === Include macros =========================================================

#[test]
fn lex_include_macros() {
    let stream = lex("locate_bytes!(\"font.chr\")");
    assert_tok(&stream, 0, "Include_locate_bytes", "locate_bytes");
    assert_tok(&stream, 1, "Op_lparen", "(");
    assert_tok(&stream, 2, "STRING", "\"font.chr\"");
    assert_tok(&stream, 3, "Op_rparen", ")");

    let stream = lex("locate_fn!(nes_code::main)");
    assert_tok(&stream, 0, "Include_locate_fn", "locate_fn");
}

// === Comments ===============================================================

#[test]
fn lex_line_comment_skipped() {
    let stream = lex("// this is a comment\nfn");
    assert_eq!(stream.tokens.len(), 1);
    assert_tok(&stream, 0, "Kw_fn", "fn");
}

#[test]
fn lex_block_comment_skipped() {
    let stream = lex("/* block comment */ fn");
    assert_eq!(stream.tokens.len(), 1);
    assert_tok(&stream, 0, "Kw_fn", "fn");
}

#[test]
fn lex_multiline_block_comment_skipped() {
    let src = "/* line 1\nline 2\nline 3 */\nfn";
    let stream = lex(src);
    assert_eq!(stream.tokens.len(), 1);
    assert_tok(&stream, 0, "Kw_fn", "fn");
}

#[test]
fn lex_doc_comment_skipped() {
    let stream = lex("/// doc comment\nfn");
    assert_eq!(stream.tokens.len(), 1);
    assert_tok(&stream, 0, "Kw_fn", "fn");
}

#[test]
fn lex_module_doc_comment_skipped() {
    let stream = lex("//! module doc\nfn");
    assert_eq!(stream.tokens.len(), 1);
    assert_tok(&stream, 0, "Kw_fn", "fn");
}

// === Line and column tracking ==============================================

#[test]
fn lex_line_col_tracking() {
    let src = "fn main()\n{\n    lda 0\n}";
    let stream = lex(src);

    // Line 1: fn main()
    assert_tok(&stream, 0, "Kw_fn", "fn");
    assert_eq!(stream.tokens[0].line, 1);
    assert_eq!(stream.tokens[0].col, 1);

    assert_tok(&stream, 1, "IDENT", "main");
    assert_eq!(stream.tokens[1].line, 1);
    assert_eq!(stream.tokens[1].col, 4);

    assert_tok(&stream, 2, "Op_lparen", "(");
    assert_eq!(stream.tokens[2].line, 1);
    assert_eq!(stream.tokens[2].col, 8);

    assert_tok(&stream, 3, "Op_rparen", ")");
    assert_eq!(stream.tokens[3].line, 1);
    assert_eq!(stream.tokens[3].col, 9);

    // Line 2: {
    assert_tok(&stream, 4, "Op_lbrace", "{");
    assert_eq!(stream.tokens[4].line, 2);
    assert_eq!(stream.tokens[4].col, 1);

    // Line 3:     lda 0
    assert_tok(&stream, 5, "OPCODE", "lda");
    assert_eq!(stream.tokens[5].line, 3);
    assert_eq!(stream.tokens[5].col, 5);

    assert_tok(&stream, 6, "NUMBER", "0");
    assert_eq!(stream.tokens[6].line, 3);
    assert_eq!(stream.tokens[6].col, 9);

    // Line 4: }
    assert_tok(&stream, 7, "Op_rbrace", "}");
    assert_eq!(stream.tokens[7].line, 4);
    assert_eq!(stream.tokens[7].col, 1);
}

// === Attributes =============================================================

#[test]
fn lex_attribute() {
    let src = "#[cfg(cpu = \"mos6502\")]";
    let stream = lex(src);
    assert_tok(&stream, 0, "Op_hash", "#");
    assert_tok(&stream, 1, "Op_lbracket", "[");
    assert_tok(&stream, 2, "IDENT", "cfg");
    assert_tok(&stream, 3, "Op_lparen", "(");
    assert_tok(&stream, 4, "IDENT", "cpu");
    assert_tok(&stream, 5, "Op_assign", "=");
    assert_tok(&stream, 6, "STRING", "\"mos6502\"");
    assert_tok(&stream, 7, "Op_rparen", ")");
    assert_tok(&stream, 8, "Op_rbracket", "]");
}

// === Full source excerpt ===================================================

#[test]
fn lex_fn_declaration() {
    let src = "fn main() { lda 0 }";
    let stream = lex(src);
    assert_tok(&stream, 0, "Kw_fn", "fn");
    assert_tok(&stream, 1, "IDENT", "main");
    assert_tok(&stream, 2, "Op_lparen", "(");
    assert_tok(&stream, 3, "Op_rparen", ")");
    assert_tok(&stream, 4, "Op_lbrace", "{");
    assert_tok(&stream, 5, "OPCODE", "lda");
    assert_tok(&stream, 6, "NUMBER", "0");
    assert_tok(&stream, 7, "Op_rbrace", "}");
}

#[test]
fn lex_inline_fn_with_params() {
    let src = "inline fn assign(dest, value) { lda value }";
    let stream = lex(src);
    assert_tok(&stream, 0, "Kw_inline", "inline");
    assert_tok(&stream, 1, "Kw_fn", "fn");
    assert_tok(&stream, 2, "IDENT", "assign");
    assert_tok(&stream, 3, "Op_lparen", "(");
    assert_tok(&stream, 4, "IDENT", "dest");
    assert_tok(&stream, 5, "Op_comma", ",");
    assert_tok(&stream, 6, "IDENT", "value");
    assert_tok(&stream, 7, "Op_rparen", ")");
}

#[test]
fn lex_use_declaration() {
    let src = "use std::cpu::*;";
    let stream = lex(src);
    assert_tok(&stream, 0, "Kw_use", "use");
    assert_tok(&stream, 1, "IDENT", "std");
    assert_tok(&stream, 2, "Op_colon_colon", "::");
    assert_tok(&stream, 3, "IDENT", "cpu");
    assert_tok(&stream, 4, "Op_colon_colon", "::");
    assert_tok(&stream, 5, "Op_star", "*");
    assert_tok(&stream, 6, "Op_semicolon", ";");
}

#[test]
fn lex_const_declaration() {
    let src = "const REFRESH_HZ: u8 = 60;";
    let stream = lex(src);
    assert_tok(&stream, 0, "Kw_const", "const");
    assert_tok(&stream, 1, "IDENT", "REFRESH_HZ");
    assert_tok(&stream, 2, "Op_colon", ":");
    assert_tok(&stream, 3, "Type_u8", "u8");
    assert_tok(&stream, 4, "Op_assign", "=");
    assert_tok(&stream, 5, "NUMBER", "60");
    assert_tok(&stream, 6, "Op_semicolon", ";");
}

#[test]
fn lex_if_statement() {
    let src = "if (set) { lda 0 }";
    let stream = lex(src);
    assert_tok(&stream, 0, "Kw_if", "if");
    assert_tok(&stream, 1, "Op_lparen", "(");
    assert_tok(&stream, 2, "Cond_set", "set");
    assert_tok(&stream, 3, "Op_rparen", ")");
    assert_tok(&stream, 4, "Op_lbrace", "{");
    assert_tok(&stream, 5, "OPCODE", "lda");
    assert_tok(&stream, 6, "NUMBER", "0");
    assert_tok(&stream, 7, "Op_rbrace", "}");
}

#[test]
fn lex_do_while_with_modifier() {
    let src = "do { lda PPU::STATUS } while (is plus)";
    let stream = lex(src);
    assert_tok(&stream, 0, "Kw_do", "do");
    assert_tok(&stream, 1, "Op_lbrace", "{");
    assert_tok(&stream, 2, "OPCODE", "lda");
    assert_tok(&stream, 3, "IDENT", "PPU");
    assert_tok(&stream, 4, "Op_colon_colon", "::");
    assert_tok(&stream, 5, "IDENT", "STATUS");
    assert_tok(&stream, 6, "Op_rbrace", "}");
    assert_tok(&stream, 7, "Kw_while", "while");
    assert_tok(&stream, 8, "Op_lparen", "(");
    assert_tok(&stream, 9, "Mod_is", "is");
    assert_tok(&stream, 10, "Cond_plus", "plus");
    assert_tok(&stream, 11, "Op_rparen", ")");
}

// === Invalid characters ====================================================

#[test]
fn lex_invalid_char_produces_diagnostic() {
    let (_stream, diags) = lex_source("test.op", "$");
    assert!(
        diags.iter().any(|d| d.code == 100),
        "expected error code 100 for invalid character"
    );
}

#[test]
fn lex_backtick_produces_diagnostic() {
    let (_stream, diags) = lex_source("test.op", "`");
    assert!(
        diags.iter().any(|d| d.code == 100),
        "expected error code 100 for backtick"
    );
}

// === Empty source ==========================================================

#[test]
fn lex_empty_source() {
    let stream = lex("");
    assert_eq!(stream.tokens.len(), 0);
}

#[test]
fn lex_whitespace_only() {
    let stream = lex("   \n\n\t  \n");
    assert_eq!(stream.tokens.len(), 0);
}

// === Bang after non-macro identifier =======================================

#[test]
fn lex_bang_after_non_macro_identifier() {
    // "foo!" where foo is not a macro name should emit IDENT then Op_bang
    let stream = lex("foo!");
    assert_eq!(stream.tokens.len(), 2);
    assert_tok(&stream, 0, "IDENT", "foo");
    assert_tok(&stream, 1, "Op_bang", "!");
}
