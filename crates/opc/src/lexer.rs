//! Stage 1: lexer.
//!
//! The lexer reads the source file as UTF-8 text and splits it into tokens.
//! It discards whitespace and comments, records the line and column of each
//! token, and writes the token stream as JSON (`.opx`) when the `--lex` stage
//! flag is set.
//!
//! The lexer hard-codes all CPU opcode sets, condition keywords, mode
//! prefixes, and compile-time macro names. It does not load definitions
//! from external libs at runtime.

use anyhow::Result;
use op_common::{Token, TokenStream, TokenType};
use op_diagnostics::Diagnostic;

use crate::cli::OpcArgs;

// --- Static classification tables -------------------------------------------

/// All Op keyword strings and their corresponding token types.
const KEYWORDS: &[(&str, TokenType)] = &[
    ("fn", TokenType::Kw_fn),
    ("inline", TokenType::Kw_inline),
    ("noreturn", TokenType::Kw_noreturn),
    ("return", TokenType::Kw_return),
    ("volatile", TokenType::Kw_volatile),
    ("struct", TokenType::Kw_struct),
    ("type", TokenType::Kw_type),
    ("enum", TokenType::Kw_enum),
    ("const", TokenType::Kw_const),
    ("mod", TokenType::Kw_mod),
    ("use", TokenType::Kw_use),
    ("pub", TokenType::Kw_pub),
    ("if", TokenType::Kw_if),
    ("else", TokenType::Kw_else),
    ("while", TokenType::Kw_while),
    ("do", TokenType::Kw_do),
    ("loop", TokenType::Kw_loop),
    ("switch", TokenType::Kw_switch),
    ("case", TokenType::Kw_case),
    ("default", TokenType::Kw_default),
    ("near", TokenType::Kw_near),
    ("far", TokenType::Kw_far),
    ("as", TokenType::Kw_as),
    ("lib", TokenType::Kw_lib),
    ("self", TokenType::Kw_self_),
    ("super", TokenType::Kw_super),
    ("true", TokenType::Kw_true),
    ("false", TokenType::Kw_false),
];

/// All primitive type strings and their corresponding token types.
const PRIMITIVE_TYPES: &[(&str, TokenType)] = &[
    ("u8", TokenType::Type_u8),
    ("i8", TokenType::Type_i8),
    ("u16", TokenType::Type_u16),
    ("i16", TokenType::Type_i16),
    ("u32", TokenType::Type_u32),
    ("i32", TokenType::Type_i32),
    ("bool", TokenType::Type_bool),
    ("pointer", TokenType::Type_pointer),
];

/// All condition keyword strings and their corresponding token types.
const CONDITION_KEYWORDS: &[(&str, TokenType)] = &[
    ("plus", TokenType::Cond_plus),
    ("positive", TokenType::Cond_positive),
    ("minus", TokenType::Cond_minus),
    ("negative", TokenType::Cond_negative),
    ("greater", TokenType::Cond_greater),
    ("less", TokenType::Cond_less),
    ("overflow", TokenType::Cond_overflow),
    ("carry", TokenType::Cond_carry),
    ("nonzero", TokenType::Cond_nonzero),
    ("set", TokenType::Cond_set),
    ("zero", TokenType::Cond_zero),
    ("unset", TokenType::Cond_unset),
    ("clear", TokenType::Cond_clear),
    ("equal", TokenType::Cond_equal),
    ("high", TokenType::Cond_high),
    ("low_or_same", TokenType::Cond_low_or_same),
    ("carry_clear", TokenType::Cond_carry_clear),
    ("carry_set", TokenType::Cond_carry_set),
    ("not_equal", TokenType::Cond_not_equal),
    ("overflow_clear", TokenType::Cond_overflow_clear),
    ("overflow_set", TokenType::Cond_overflow_set),
    ("greater_or_equal", TokenType::Cond_greater_or_equal),
    ("less_than", TokenType::Cond_less_than),
    ("greater_than", TokenType::Cond_greater_than),
    ("less_or_equal", TokenType::Cond_less_or_equal),
    ("not_zero", TokenType::Cond_not_zero),
    ("no_carry", TokenType::Cond_no_carry),
    ("parity_even", TokenType::Cond_parity_even),
    ("parity_odd", TokenType::Cond_parity_odd),
    ("sign_positive", TokenType::Cond_sign_positive),
    ("sign_negative", TokenType::Cond_sign_negative),
];

/// Condition modifier keyword strings.
const CONDITION_MODIFIERS: &[(&str, TokenType)] = &[
    ("is", TokenType::Mod_is),
    ("has", TokenType::Mod_has),
    ("no", TokenType::Mod_no),
    ("not", TokenType::Mod_not),
];

/// Mode prefix strings and their corresponding token types.
const MODE_PREFIXES: &[(&str, TokenType)] = &[
    ("zp", TokenType::Mode_zp),
    ("abs", TokenType::Mode_abs),
    ("rel", TokenType::Mode_rel),
    ("ind", TokenType::Mode_ind),
    ("idx", TokenType::Mode_idx),
    ("ind_l", TokenType::Mode_ind_l),
    ("ind_idx", TokenType::Mode_ind_idx),
];

/// Compile-time macro names and their corresponding token types.
const COMPILE_MACROS: &[(&str, TokenType)] = &[
    ("lo", TokenType::Macro_lo),
    ("hi", TokenType::Macro_hi),
    ("nylo", TokenType::Macro_nylo),
    ("nyhi", TokenType::Macro_nyhi),
    ("sizeof", TokenType::Macro_sizeof),
];

/// Include macro names and their corresponding token types.
const INCLUDE_MACROS: &[(&str, TokenType)] = &[
    ("locate_bytes", TokenType::Include_locate_bytes),
    ("locate_str", TokenType::Include_locate_str),
    ("locate_fn", TokenType::Include_locate_fn),
];

/// All CPU opcode mnemonics in lowercase. The lexer matches opcodes
/// case-insensitively. This includes the 6502, 65SC02, 65C816, 68000,
/// Z80, and LR35902 CPU families.
const OPCODES: &[&str] = &[
    // 6502
    "adc", "and", "asl", "bcc", "bcs", "beq", "bit", "bmi", "bne", "bpl", "brk", "bvc", "bvs",
    "clc", "cld", "cli", "clv", "cmp", "cpx", "cpy", "dec", "dex", "dey", "eor", "inc", "inx",
    "iny", "jmp", "jsr", "lda", "ldx", "ldy", "lsr", "nop", "ora", "pha", "php", "pla", "plp",
    "rol", "ror", "rti", "rts", "sbc", "sec", "sed", "sei", "sta", "stx", "sty", "tax", "tay",
    "tsx", "txa", "txs", "tya", // 6502 undocumented
    "alr", "anc", "ane", "arr", "dcp", "isc", "las", "lax", "lxa", "rla", "rra", "sax", "sha",
    "shx", "shy", "slo", "sre", "tas", "usbc", // 65SC02
    "bra", "phx", "phy", "plx", "ply", "stz", "tsb", "trb", "ina", "dea", // 65C816
    "rep", "sep", "xba", "xce", "tcd", "tdc", "tcs", "tsc", "txy", "tyx", "mvn", "mvp", "pea",
    "pei", "per", "jml", "jsl", "rtl", "cop", "wai", "stp", // 68000
    "move", "moveq", "movem", "lea", "clr", "not", "or", "eor", "add", "adda", "addi", "addq",
    "sub", "suba", "subi", "subq", "mulu", "muls", "divu", "divs", "neg", "negx", "abs", "asr",
    "lsl", "lsr", "ror", "roxl", "roxr", "cmpa", "cmpi", "tst", "btst", "bset", "bclr", "bchg",
    "rtr", "rte", "bsr", "dbcc", "chk", "trap", "trapv", "swap", "exg", "ext", "link", "unlk",
    "reset", "stop", "illegal", // Z80
    "ld", "push", "pop", "ex", "exx", "ldi", "ldir", "ldd", "lddr", "cpi", "cpir", "cpd", "cpdr",
    "sbc", "cp", "inc", "dec", "daa", "cpl", "ccf", "scf", "halt", "di", "ei", "im", "rlc", "rl",
    "rrc", "rr", "sla", "sra", "sll", "srl", "rld", "rrd", "rlca", "rrca", "rra", "jp", "jr",
    "djnz", "call", "ret", "reti", "retn", "rst", "in", "out", "ini", "inir", "ind", "indr",
    "outi", "otir", "outd", "otdr", "bit", "set", "res", // LR35902
    "stop", "ldh",
];

/// Multi-character operators, sorted longest-first for longest-match.
const MULTI_CHAR_OPS: &[(&str, TokenType)] = &[
    ("::", TokenType::Op_colon_colon),
    (">>", TokenType::Op_shr),
    ("<<", TokenType::Op_shl),
    (">=", TokenType::Op_ge),
    ("<=", TokenType::Op_le),
    ("==", TokenType::Op_eq),
    ("!=", TokenType::Op_ne),
];

/// Single-character operators and punctuation.
const SINGLE_CHAR_OPS: &[(char, TokenType)] = &[
    ('#', TokenType::Op_hash),
    ('.', TokenType::Op_dot),
    ('+', TokenType::Op_plus),
    ('-', TokenType::Op_minus),
    ('*', TokenType::Op_star),
    ('/', TokenType::Op_slash),
    ('%', TokenType::Op_percent),
    ('~', TokenType::Op_tilde),
    ('!', TokenType::Op_bang),
    ('&', TokenType::Op_amp),
    ('^', TokenType::Op_caret),
    ('|', TokenType::Op_pipe),
    ('>', TokenType::Op_gt),
    ('<', TokenType::Op_lt),
    ('=', TokenType::Op_assign),
    ('(', TokenType::Op_lparen),
    (')', TokenType::Op_rparen),
    ('{', TokenType::Op_lbrace),
    ('}', TokenType::Op_rbrace),
    ('[', TokenType::Op_lbracket),
    (']', TokenType::Op_rbracket),
    (':', TokenType::Op_colon),
    (',', TokenType::Op_comma),
    (';', TokenType::Op_semicolon),
];

/// Valid string escape characters.
const STRING_ESCAPES: &[char] = &['n', 'r', 't', '0', 'a', '\\', '"'];

// --- Lexer entry points -----------------------------------------------------

/// Run the lexer stage when the `--lex` flag is set, or no-op otherwise.
pub fn run(args: &OpcArgs) -> Result<()> {
    if !args.lex {
        return Ok(());
    }
    let stream = lex_file(&args.input.input)?;
    let json = op_common::to_json(&stream)?;
    match &args.output {
        Some(path) => std::fs::write(path, json)?,
        None => println!("{json}"),
    }
    Ok(())
}

/// Lex a source file into a [`TokenStream`]. Returns an error if the
/// lexer produces any error diagnostics.
pub fn lex_file(path: &str) -> Result<TokenStream> {
    let source = std::fs::read_to_string(path)?;
    let (stream, diags) = lex_source(path, &source);
    let has_errors = diags
        .iter()
        .any(|d| d.severity == op_diagnostics::Severity::Error);
    if has_errors {
        for d in &diags {
            d.print(None);
        }
        anyhow::bail!("lexer errors in {}", path);
    }
    Ok(stream)
}

/// Lex a source string into a [`TokenStream`] and a list of diagnostics.
///
/// This is the main entry point for the tokenizer. It scans the source
/// character by character, classifies each token, and records the line
/// and column of each token.
pub fn lex_source(file: &str, source: &str) -> (TokenStream, Vec<Diagnostic>) {
    let mut stream = TokenStream::new(file);
    let mut diags = Vec::new();
    let mut pos = 0usize;
    let mut line = 1u32;
    let mut col = 1u32;

    let chars: Vec<char> = source.chars().collect();

    while pos < chars.len() {
        let c = chars[pos];

        // Skip whitespace.
        if c == ' ' || c == '\t' || c == '\r' {
            advance(&mut pos, &mut col);
            continue;
        }
        if c == '\n' {
            advance(&mut pos, &mut col);
            line += 1;
            col = 1;
            continue;
        }

        // Skip comments.
        if c == '/' && pos + 1 < chars.len() {
            // Line comment: // to end of line
            if chars[pos + 1] == '/' {
                while pos < chars.len() && chars[pos] != '\n' {
                    advance(&mut pos, &mut col);
                }
                continue;
            }
            // Block comment: /* to */
            if chars[pos + 1] == '*' {
                advance(&mut pos, &mut col); // /
                advance(&mut pos, &mut col); // *
                while pos + 1 < chars.len() {
                    if chars[pos] == '*' && chars[pos + 1] == '/' {
                        advance(&mut pos, &mut col); // *
                        advance(&mut pos, &mut col); // /
                        break;
                    }
                    if chars[pos] == '\n' {
                        line += 1;
                        col = 1;
                        pos += 1;
                    } else {
                        advance(&mut pos, &mut col);
                    }
                }
                continue;
            }
        }

        // Multi-character operators (longest match first).
        let token_start = pos;
        let token_line = line;
        let token_col = col;
        let mut matched = false;

        for (op_text, op_type) in MULTI_CHAR_OPS {
            let op_chars: Vec<char> = op_text.chars().collect();
            if pos + op_chars.len() <= chars.len() {
                if chars[pos..pos + op_chars.len()] == op_chars[..] {
                    push_token(&mut stream, *op_type, op_text, token_line, token_col);
                    for _ in &op_chars {
                        advance(&mut pos, &mut col);
                    }
                    matched = true;
                    break;
                }
            }
        }
        if matched {
            continue;
        }

        // Single-character operators and punctuation.
        for (op_char, op_type) in SINGLE_CHAR_OPS {
            if c == *op_char {
                push_token(&mut stream, *op_type, &c.to_string(), token_line, token_col);
                advance(&mut pos, &mut col);
                matched = true;
                break;
            }
        }
        if matched {
            continue;
        }

        // Numbers: decimal, binary (%), hex (0x).
        if c == '%' || c.is_ascii_digit() {
            let start = pos;
            if c == '%' {
                advance(&mut pos, &mut col);
                while pos < chars.len() && chars[pos].is_ascii_digit() {
                    // binary digits are 0 and 1
                    if chars[pos] != '0' && chars[pos] != '1' {
                        break;
                    }
                    advance(&mut pos, &mut col);
                }
            } else if c == '0' && pos + 1 < chars.len() && chars[pos + 1] == 'x' {
                advance(&mut pos, &mut col); // 0
                advance(&mut pos, &mut col); // x
                while pos < chars.len() && chars[pos].is_ascii_hexdigit() {
                    advance(&mut pos, &mut col);
                }
            } else {
                // decimal
                while pos < chars.len() && chars[pos].is_ascii_digit() {
                    advance(&mut pos, &mut col);
                }
            }
            let value: String = chars[start..pos].iter().collect();
            push_token(
                &mut stream,
                TokenType::Number,
                &value,
                token_line,
                token_col,
            );
            continue;
        }

        // Strings.
        if c == '"' {
            advance(&mut pos, &mut col); // opening "
            while pos < chars.len() && chars[pos] != '"' {
                if chars[pos] == '\\' {
                    if pos + 1 >= chars.len() {
                        diags.push(Diagnostic::error(
                            101,
                            file,
                            line,
                            col,
                            "unterminated string escape",
                        ));
                        break;
                    }
                    let esc = chars[pos + 1];
                    if !STRING_ESCAPES.contains(&esc) {
                        diags.push(Diagnostic::error(
                            102,
                            file,
                            line,
                            col,
                            format!("invalid escape sequence \\{}", esc),
                        ));
                    }
                    advance(&mut pos, &mut col); // backslash
                    advance(&mut pos, &mut col); // escape char
                } else {
                    if chars[pos] == '\n' {
                        line += 1;
                        col = 1;
                        pos += 1;
                    } else {
                        advance(&mut pos, &mut col);
                    }
                }
            }
            if pos < chars.len() && chars[pos] == '"' {
                advance(&mut pos, &mut col); // closing "
            } else {
                diags.push(Diagnostic::error(
                    103,
                    file,
                    token_line,
                    token_col,
                    "unterminated string literal",
                ));
            }
            let value: String = chars[token_start..pos].iter().collect();
            push_token(
                &mut stream,
                TokenType::String_,
                &value,
                token_line,
                token_col,
            );
            continue;
        }

        // Labels: 'identifier or 'identifier:
        if c == '\'' {
            advance(&mut pos, &mut col); // '
                                         // Read the identifier after the quote.
            if pos < chars.len() && (chars[pos].is_ascii_alphabetic() || chars[pos] == '_') {
                advance(&mut pos, &mut col);
                while pos < chars.len() && (chars[pos].is_ascii_alphanumeric() || chars[pos] == '_')
                {
                    advance(&mut pos, &mut col);
                }
            }
            // Check for colon (label definition).
            if pos < chars.len() && chars[pos] == ':' {
                advance(&mut pos, &mut col); // :
                let value: String = chars[token_start..pos].iter().collect();
                push_token(
                    &mut stream,
                    TokenType::LabelDef,
                    &value,
                    token_line,
                    token_col,
                );
            } else {
                let value: String = chars[token_start..pos].iter().collect();
                push_token(
                    &mut stream,
                    TokenType::LabelRef,
                    &value,
                    token_line,
                    token_col,
                );
            }
            continue;
        }

        // Identifiers and keywords.
        if c.is_ascii_alphabetic() || c == '_' {
            let start = pos;
            advance(&mut pos, &mut col);
            while pos < chars.len() && (chars[pos].is_ascii_alphanumeric() || chars[pos] == '_') {
                advance(&mut pos, &mut col);
            }
            let word: String = chars[start..pos].iter().collect();

            // Classify the identifier.
            let token_type = classify_identifier(
                &word,
                pos < chars.len() && chars[pos] == '!',
                &mut pos,
                &mut col,
            );

            push_token(&mut stream, token_type, &word, token_line, token_col);
            continue;
        }

        // Unknown character: report an error.
        diags.push(Diagnostic::error(
            100,
            file,
            line,
            col,
            format!("unexpected character {:?}", c),
        ));
        advance(&mut pos, &mut col);
    }

    (stream, diags)
}

/// Classify an identifier and optionally consume a trailing `!` for macros.
fn classify_identifier(word: &str, has_bang: bool, pos: &mut usize, _col: &mut u32) -> TokenType {
    // Check keywords.
    for (kw, tt) in KEYWORDS {
        if word == *kw {
            return *tt;
        }
    }

    // Check primitive types.
    for (ty, tt) in PRIMITIVE_TYPES {
        if word == *ty {
            return *tt;
        }
    }

    // Check condition keywords.
    for (cond, tt) in CONDITION_KEYWORDS {
        if word == *cond {
            return *tt;
        }
    }

    // Check condition modifiers.
    for (mod_, tt) in CONDITION_MODIFIERS {
        if word == *mod_ {
            return *tt;
        }
    }

    // Check mode prefixes.
    for (mode, tt) in MODE_PREFIXES {
        if word == *mode {
            return *tt;
        }
    }

    // Check compile-time macros (followed by !).
    if has_bang {
        for (mac, tt) in COMPILE_MACROS {
            if word == *mac {
                *pos += 1; // consume the !
                return *tt;
            }
        }
        for (mac, tt) in INCLUDE_MACROS {
            if word == *mac {
                *pos += 1; // consume the !
                return *tt;
            }
        }
    }

    // Check opcodes (case-insensitive).
    let lower = word.to_ascii_lowercase();
    if OPCODES.contains(&lower.as_str()) {
        return TokenType::Opcode;
    }

    TokenType::Ident
}

/// Push a token onto the stream.
fn push_token(stream: &mut TokenStream, tt: TokenType, value: &str, line: u32, col: u32) {
    stream.tokens.push(Token {
        kind: tt.as_str().to_string(),
        value: value.to_string(),
        line,
        col,
    });
}

/// Advance the position and column by one.
fn advance(pos: &mut usize, col: &mut u32) {
    *pos += 1;
    *col += 1;
}
