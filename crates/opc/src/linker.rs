//! Stage 4: linker.
//!
//! The linker reads the JSON object data (`.opl` post-compile), resolves all
//! relocations, lays out the sections in the final memory map, writes the
//! interrupt vector table, writes the output file header, pads each section,
//! and writes the final linked data (`.opl` post-link).
//!
//! The full linker uses the target memory map and the recorded header fields.
//! This module provides the scaffolding for that logic.

use anyhow::Result;
use op_ir::ObjectFile;

use crate::cli::OpcArgs;

/// Run the linker stage when the `--link` flag is set.
pub fn run(args: &OpcArgs) -> Result<()> {
    if !args.link {
        return Ok(());
    }
    let linked = link_file(&args.input.input, args.target.as_deref().unwrap_or(""))?;
    let json = op_common::to_json(&linked)?;
    match &args.output {
        Some(path) => std::fs::write(path, json)?,
        None => println!("{json}"),
    }
    Ok(())
}

/// Link an object file into a final [`ObjectFile`] with resolved relocations.
pub fn link_file(_path: &str, target: &str) -> Result<ObjectFile> {
    Ok(ObjectFile::new(target))
}