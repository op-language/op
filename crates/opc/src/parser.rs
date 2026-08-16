//! Stage 2: parser.
//!
//! The parser reads the JSON token stream (`.opx`) or the source file, builds
//! the AST that the normalized LR(1) grammar defines, evaluates `#[cfg]`
//! attributes and `const` expressions, resolves `mod` declarations, and writes
//! the AST as JSON (`.opa`) when the `--parse` stage flag is set.
//!
//! The full parser uses the `lalrpop` crate to generate the parser from the
//! grammar. This module provides the scaffolding for that logic.

use anyhow::Result;
use op_common::{ast::Module, AstFile};

use crate::cli::OpcArgs;

/// Run the parser stage when the `--parse` flag is set, or no-op otherwise.
pub fn run(args: &OpcArgs) -> Result<()> {
    if !args.parse {
        return Ok(());
    }
    let ast = parse_file(&args.input.input, args.target.as_deref().unwrap_or(""))?;
    let json = op_common::to_json(&ast)?;
    match &args.output {
        Some(path) => std::fs::write(path, json)?,
        None => println!("{json}"),
    }
    Ok(())
}

/// Parse a source file into an [`AstFile`].
pub fn parse_file(path: &str, target: &str) -> Result<AstFile> {
    let module = Module::new(path.strip_suffix(".op").unwrap_or(path).to_string());
    Ok(AstFile {
        version: 1,
        target: target.to_string(),
        root: module,
    })
}
