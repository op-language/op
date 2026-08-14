//! Token types for the lexer stage.
//!
//! The lexer emits the [`TokenStream`] described in the technical design
//! section "Stage 1: lexer". Each [`Token`] records its type, its source
//! text, and the line and column where it starts.

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