//! Command-line interface for `opc`.
//!
//! Implements the option set defined in the technical design section "opc
//! binary / Command-line interface". The four stage flags are mutually
//! exclusive.

use anyhow::Result;
use clap::{ArgAction, Args, Parser};

/// The Op compiler.
#[derive(Debug, Parser)]
#[command(name = "opc", version, about, long_about = None)]
pub struct OpcArgs {
    /// Run the lexer only. Write a .opx file.
    #[arg(long, group = "stage")]
    pub lex: bool,

    /// Run the lexer and parser. Write a .opa file.
    #[arg(long, group = "stage")]
    pub parse: bool,

    /// Run lexer, parser, codegen, optimizer. Write a .opl file.
    #[arg(long, group = "stage")]
    pub compile: bool,

    /// Run all stages through linker. Write a .opl file.
    #[arg(long, group = "stage")]
    pub link: bool,

    /// Output file path.
    #[arg(short = 'o')]
    pub output: Option<String>,

    /// Output format for final ROM (ines, lnx, raw, hex).
    #[arg(long)]
    pub format: Option<String>,

    /// Target triplet, e.g. mos6502-nintendo-nes-ntsc.
    #[arg(long)]
    pub target: Option<String>,

    /// CPU family name (overrides triplet CPU).
    #[arg(long)]
    pub cpu: Option<String>,

    /// Enable a feature flag.
    #[arg(long = "feature", action = ArgAction::Append)]
    pub features: Vec<String>,

    /// Add a directory to the include search path.
    #[arg(short = 'I', action = ArgAction::Append)]
    pub include: Vec<String>,

    /// Optimization level (0 = none, 1 = keyhole peephole). Default: 1.
    #[arg(short = 'O', default_value = "1")]
    pub opt_level: u8,

    /// Stop after n errors. Default: 20.
    #[arg(long, default_value = "20")]
    pub error_limit: u32,

    #[command(flatten)]
    pub input: Input,
}

#[derive(Debug, Args)]
pub struct Input {
    /// Input source file (or intermediate file for stage flags).
    pub input: String,
}

/// Entry point for the `opc` CLI.
pub fn run() -> Result<()> {
    let args = OpcArgs::parse();
    crate::lexer::run(&args)?;
    crate::parser::run(&args)?;
    crate::codegen::run(&args)?;
    crate::linker::run(&args)?;
    crate::output::run(&args)?;
    Ok(())
}
