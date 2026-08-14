//! Stage 5: file output.
//!
//! When `opc` runs without a stage flag, the file output stage reads the
//! linked data and writes the final ROM or binary image. The output format
//! depends on the target or the `--format` flag: `ines`, `lnx`, `raw`, or
//! `hex`.

use anyhow::Result;

use crate::cli::OpcArgs;

/// Run the file output stage when no stage flag is set.
pub fn run(args: &OpcArgs) -> Result<()> {
    if args.lex || args.parse || args.compile || args.link {
        return Ok(());
    }
    let format = args.format.as_deref().unwrap_or("raw");
    let bytes = emit(args, format)?;
    match &args.output {
        Some(path) => std::fs::write(path, bytes)?,
        None => {
            use std::io::Write;
            std::io::stdout().write_all(&bytes)?;
        }
    }
    Ok(())
}

/// Emit the final binary image bytes for the given format.
fn emit(_args: &OpcArgs, _format: &str) -> Result<Vec<u8>> {
    Ok(Vec::new())
}