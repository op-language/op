//! Token types for the lexer stage.
//!
//! The lexer emits the [`TokenStream`] described in the technical design
//! section "Stage 1: lexer". Each [`Token`] records its type, its source
//! text, and the line and column where it starts.
//!
//! The [`TokenType`] enum lists every token type the lexer can produce.
//! The lexer uses `TokenType::as_str()` to populate the `Token.kind`
//! field so the JSON `.opx` format uses the string name.

use serde::{Deserialize, Serialize};

use crate::envelope::Envelope;

/// A single token produced by the lexer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Token {
    #[serde(rename = "type")]
    pub kind: String,
    pub value: String,
    pub line: u32,
    pub col: u32,
}

/// Every token type the Op lexer can produce.
///
/// The lexer uses [`TokenType::as_str`] to set the `Token.kind` string
/// field. The string names match the token type reference table in
/// `docs/file-formats.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(non_camel_case_types)]
pub enum TokenType {
    // --- Keywords --------------------------------------------------------
    Kw_fn,
    Kw_inline,
    Kw_noreturn,
    Kw_return,
    Kw_volatile,
    Kw_struct,
    Kw_type,
    Kw_enum,
    Kw_const,
    Kw_mod,
    Kw_use,
    Kw_pub,
    Kw_if,
    Kw_else,
    Kw_while,
    Kw_do,
    Kw_loop,
    Kw_switch,
    Kw_case,
    Kw_default,
    Kw_near,
    Kw_far,
    Kw_as,
    Kw_lib,
    Kw_self_,
    Kw_super,
    Kw_true,
    Kw_false,

    // --- Primitive types -------------------------------------------------
    Type_u8,
    Type_i8,
    Type_u16,
    Type_i16,
    Type_u32,
    Type_i32,
    Type_bool,
    Type_pointer,

    // --- Operators and punctuation --------------------------------------
    Op_hash,
    Op_colon_colon,
    Op_dot,
    Op_plus,
    Op_minus,
    Op_star,
    Op_slash,
    Op_percent,
    Op_tilde,
    Op_bang,
    Op_amp,
    Op_caret,
    Op_pipe,
    Op_shr,
    Op_shl,
    Op_gt,
    Op_lt,
    Op_ge,
    Op_le,
    Op_eq,
    Op_ne,
    Op_assign,
    Op_lparen,
    Op_rparen,
    Op_lbrace,
    Op_rbrace,
    Op_lbracket,
    Op_rbracket,
    Op_colon,
    Op_comma,
    Op_semicolon,

    // --- Literals and identifiers ---------------------------------------
    Number,
    String_,
    Ident,
    Opcode,
    LabelDef,
    LabelRef,

    // --- Compile-time macros --------------------------------------------
    Macro_lo,
    Macro_hi,
    Macro_nylo,
    Macro_nyhi,
    Macro_sizeof,

    // --- Include macros -------------------------------------------------
    Include_locate_bytes,
    Include_locate_str,
    Include_locate_fn,

    // --- Condition keywords ---------------------------------------------
    Cond_plus,
    Cond_positive,
    Cond_minus,
    Cond_negative,
    Cond_greater,
    Cond_less,
    Cond_overflow,
    Cond_carry,
    Cond_nonzero,
    Cond_set,
    Cond_zero,
    Cond_unset,
    Cond_clear,
    Cond_equal,
    Cond_high,
    Cond_low_or_same,
    Cond_carry_clear,
    Cond_carry_set,
    Cond_not_equal,
    Cond_overflow_clear,
    Cond_overflow_set,
    Cond_greater_or_equal,
    Cond_less_than,
    Cond_greater_than,
    Cond_less_or_equal,
    Cond_not_zero,
    Cond_no_carry,
    Cond_parity_even,
    Cond_parity_odd,
    Cond_sign_positive,
    Cond_sign_negative,
    Cond_true,
    Cond_false,

    // --- Condition modifiers --------------------------------------------
    Mod_is,
    Mod_has,
    Mod_no,
    Mod_not,

    // --- Mode prefixes --------------------------------------------------
    Mode_zp,
    Mode_abs,
    Mode_rel,
    Mode_ind,
    Mode_idx,
    Mode_ind_l,
    Mode_ind_idx,
}

impl TokenType {
    /// Return the string name of this token type.
    ///
    /// The string matches the `type` field in the `.opx` JSON format
    /// and the token type reference table in `docs/file-formats.md`.
    pub fn as_str(&self) -> &'static str {
        match self {
            // Keywords
            Self::Kw_fn => "Kw_fn",
            Self::Kw_inline => "Kw_inline",
            Self::Kw_noreturn => "Kw_noreturn",
            Self::Kw_return => "Kw_return",
            Self::Kw_volatile => "Kw_volatile",
            Self::Kw_struct => "Kw_struct",
            Self::Kw_type => "Kw_type",
            Self::Kw_enum => "Kw_enum",
            Self::Kw_const => "Kw_const",
            Self::Kw_mod => "Kw_mod",
            Self::Kw_use => "Kw_use",
            Self::Kw_pub => "Kw_pub",
            Self::Kw_if => "Kw_if",
            Self::Kw_else => "Kw_else",
            Self::Kw_while => "Kw_while",
            Self::Kw_do => "Kw_do",
            Self::Kw_loop => "Kw_loop",
            Self::Kw_switch => "Kw_switch",
            Self::Kw_case => "Kw_case",
            Self::Kw_default => "Kw_default",
            Self::Kw_near => "Kw_near",
            Self::Kw_far => "Kw_far",
            Self::Kw_as => "Kw_as",
            Self::Kw_lib => "Kw_lib",
            Self::Kw_self_ => "Kw_self",
            Self::Kw_super => "Kw_super",
            Self::Kw_true => "Kw_true",
            Self::Kw_false => "Kw_false",

            // Primitive types
            Self::Type_u8 => "Type_u8",
            Self::Type_i8 => "Type_i8",
            Self::Type_u16 => "Type_u16",
            Self::Type_i16 => "Type_i16",
            Self::Type_u32 => "Type_u32",
            Self::Type_i32 => "Type_i32",
            Self::Type_bool => "Type_bool",
            Self::Type_pointer => "Type_pointer",

            // Operators and punctuation
            Self::Op_hash => "Op_hash",
            Self::Op_colon_colon => "Op_colon_colon",
            Self::Op_dot => "Op_dot",
            Self::Op_plus => "Op_plus",
            Self::Op_minus => "Op_minus",
            Self::Op_star => "Op_star",
            Self::Op_slash => "Op_slash",
            Self::Op_percent => "Op_percent",
            Self::Op_tilde => "Op_tilde",
            Self::Op_bang => "Op_bang",
            Self::Op_amp => "Op_amp",
            Self::Op_caret => "Op_caret",
            Self::Op_pipe => "Op_pipe",
            Self::Op_shr => "Op_shr",
            Self::Op_shl => "Op_shl",
            Self::Op_gt => "Op_gt",
            Self::Op_lt => "Op_lt",
            Self::Op_ge => "Op_ge",
            Self::Op_le => "Op_le",
            Self::Op_eq => "Op_eq",
            Self::Op_ne => "Op_ne",
            Self::Op_assign => "Op_assign",
            Self::Op_lparen => "Op_lparen",
            Self::Op_rparen => "Op_rparen",
            Self::Op_lbrace => "Op_lbrace",
            Self::Op_rbrace => "Op_rbrace",
            Self::Op_lbracket => "Op_lbracket",
            Self::Op_rbracket => "Op_rbracket",
            Self::Op_colon => "Op_colon",
            Self::Op_comma => "Op_comma",
            Self::Op_semicolon => "Op_semicolon",

            // Literals and identifiers
            Self::Number => "NUMBER",
            Self::String_ => "STRING",
            Self::Ident => "IDENT",
            Self::Opcode => "OPCODE",
            Self::LabelDef => "LABEL_DEF",
            Self::LabelRef => "LABEL_REF",

            // Compile-time macros
            Self::Macro_lo => "Macro_lo",
            Self::Macro_hi => "Macro_hi",
            Self::Macro_nylo => "Macro_nylo",
            Self::Macro_nyhi => "Macro_nyhi",
            Self::Macro_sizeof => "Macro_sizeof",

            // Include macros
            Self::Include_locate_bytes => "Include_locate_bytes",
            Self::Include_locate_str => "Include_locate_str",
            Self::Include_locate_fn => "Include_locate_fn",

            // Condition keywords
            Self::Cond_plus => "Cond_plus",
            Self::Cond_positive => "Cond_positive",
            Self::Cond_minus => "Cond_minus",
            Self::Cond_negative => "Cond_negative",
            Self::Cond_greater => "Cond_greater",
            Self::Cond_less => "Cond_less",
            Self::Cond_overflow => "Cond_overflow",
            Self::Cond_carry => "Cond_carry",
            Self::Cond_nonzero => "Cond_nonzero",
            Self::Cond_set => "Cond_set",
            Self::Cond_zero => "Cond_zero",
            Self::Cond_unset => "Cond_unset",
            Self::Cond_clear => "Cond_clear",
            Self::Cond_equal => "Cond_equal",
            Self::Cond_high => "Cond_high",
            Self::Cond_low_or_same => "Cond_low_or_same",
            Self::Cond_carry_clear => "Cond_carry_clear",
            Self::Cond_carry_set => "Cond_carry_set",
            Self::Cond_not_equal => "Cond_not_equal",
            Self::Cond_overflow_clear => "Cond_overflow_clear",
            Self::Cond_overflow_set => "Cond_overflow_set",
            Self::Cond_greater_or_equal => "Cond_greater_or_equal",
            Self::Cond_less_than => "Cond_less_than",
            Self::Cond_greater_than => "Cond_greater_than",
            Self::Cond_less_or_equal => "Cond_less_or_equal",
            Self::Cond_not_zero => "Cond_not_zero",
            Self::Cond_no_carry => "Cond_no_carry",
            Self::Cond_parity_even => "Cond_parity_even",
            Self::Cond_parity_odd => "Cond_parity_odd",
            Self::Cond_sign_positive => "Cond_sign_positive",
            Self::Cond_sign_negative => "Cond_sign_negative",
            Self::Cond_true => "Cond_true",
            Self::Cond_false => "Cond_false",

            // Condition modifiers
            Self::Mod_is => "Mod_is",
            Self::Mod_has => "Mod_has",
            Self::Mod_no => "Mod_no",
            Self::Mod_not => "Mod_not",

            // Mode prefixes
            Self::Mode_zp => "Mode_zp",
            Self::Mode_abs => "Mode_abs",
            Self::Mode_rel => "Mode_rel",
            Self::Mode_ind => "Mode_ind",
            Self::Mode_idx => "Mode_idx",
            Self::Mode_ind_l => "Mode_ind_l",
            Self::Mode_ind_idx => "Mode_ind_idx",
        }
    }
}

/// The `.opx` post-lexer token stream envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenStream {
    pub version: u32,
    pub file: String,
    pub tokens: Vec<Token>,
}

impl TokenStream {
    pub fn new(file: impl Into<String>) -> Self {
        Self {
            version: 1,
            file: file.into(),
            tokens: Vec::new(),
        }
    }
}

impl Envelope for TokenStream {
    fn version(&self) -> u32 {
        self.version
    }
}
