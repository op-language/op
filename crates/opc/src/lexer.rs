//! Stage 1: lexer.
//!
//! The lexer reads the source file as UTF-8 text and splits it into tokens.
//! It discards whitespace and comments, records the line and column of each
//! token, and writes the token stream as JSON (`.opx`) when the `--lex` stage
//! flag is set.
//!
//! The full tokenizer uses the `logos` crate and loads target-specific opcode
//! mnemonics from the bank in `~/.carts/`. This module provides the scaffolding
//! for that logic.

use anyhow::Result;
use op_common::{Token, TokenStream};

use crate::cli::OpcArgs;

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

/// Lex a source file into a [`TokenStream`].
pub fn lex_file(path: &str) -> Result<TokenStream> {
    let source = std::fs::read_to_string(path)?;
    Ok(lex_source(path, &source))
}

/// Lex a source string into a [`TokenStream`].
pub fn lex_source(file: &str, source: &str) -> TokenStream {
    let mut stream = TokenStream::new(file);
    for (i, line) in source.lines().enumerate() {
        let line_no = (i + 1) as u32;
        let mut col = 1u32;
        for token in line.split_whitespace() {
            stream.tokens.push(Token {
                kind: classify(token),
                value: token.to_string(),
                line: line_no,
                col,
            });
            col += token.len() as u32 + 1;
        }
    }
    stream
}

/// Classify a raw token string. The full implementation looks up the bank
/// table; this placeholder treats every token as an identifier.
fn classify(token: &str) -> String {
    if token.chars().all(|c| c.is_ascii_digit()) {
        return "NUMBER".to_string();
    }
    "IDENT".to_string()
}