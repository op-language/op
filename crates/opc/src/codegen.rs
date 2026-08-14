//! Stage 3: code generation and optimization.
//!
//! The code generator reads the JSON AST (`.opa`), walks it, and emits an
//! object file with sections, symbols, relocations, and data bytes. The
//! keyhole peephole optimizer runs after the code generator. When the
//! `--compile` stage flag is set, `opc` writes the object data as JSON
//! (`.opl`).
//!
//! The full code generator uses the bank's opcode encoding table to encode
//! each instruction. This module provides the scaffolding for that logic.

use anyhow::Result;
use op_ir::ObjectFile;

use crate::cli::OpcArgs;

/// Run the codegen and optimizer stage when the `--compile` flag is set.
pub fn run(args: &OpcArgs) -> Result<()> {
    if !args.compile {
        return Ok(());
    }
    let obj = compile_file(&args.input.input, args.target.as_deref().unwrap_or(""))?;
    let json = op_common::to_json(&obj)?;
    match &args.output {
        Some(path) => std::fs::write(path, json)?,
        None => println!("{json}"),
    }
    Ok(())
}

/// Compile a source file (or read an `.opa` file) into an [`ObjectFile`].
pub fn compile_file(_path: &str, target: &str) -> Result<ObjectFile> {
    Ok(ObjectFile::new(target))
}