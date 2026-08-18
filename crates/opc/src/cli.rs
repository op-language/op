//! Command-line interface for `opc`.
//!
//! Implements the option set defined in the technical design section "opc
//! binary / Command-line interface". The four stage flags are mutually
//! exclusive.

use anyhow::Result;
use clap::{ArgAction, Args, Parser};
use op_diagnostics::Severity;

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

    // When a stage flag is set, run only that stage. Each stage reads its
    // own input and writes its own output.
    if args.lex {
        return crate::lexer::run(&args);
    }
    if args.parse {
        return crate::parser::run(&args);
    }
    if args.compile {
        return crate::codegen::run(&args);
    }
    if args.link {
        return crate::linker::run(&args);
    }

    // No stage flag: run the full pipeline in memory.
    run_pipeline(&args)
}

/// Run the full in-memory pipeline: lex, parse, codegen, optimize, link,
/// and emit. Each stage passes its output to the next stage in memory.
fn run_pipeline(args: &OpcArgs) -> Result<()> {
    let target = args.target.as_deref().unwrap_or("");
    let input_path = &args.input.input;

    // 1. Read the source file.
    let source = std::fs::read_to_string(input_path)
        .map_err(|e| anyhow::anyhow!("failed to read {input_path}: {e}"))?;

    // 2. Parse (parse_source calls lex_source internally).
    let (ast, parse_diags) =
        crate::parser::parse_source(input_path, &source, target, &args.features);
    print_diags(&parse_diags, input_path, &source);
    if has_errors(&parse_diags) {
        anyhow::bail!("parser errors in {input_path}");
    }

    // 3. Compile (codegen + optimizer).
    let (obj, codegen_diags) = crate::codegen::compile_source(&ast, args.opt_level);
    print_diags(&codegen_diags, input_path, &source);
    if has_errors(&codegen_diags) {
        anyhow::bail!("codegen errors in {input_path}");
    }

    // 4. Link.
    let (linked, linker_diags) = crate::linker::link_source(&obj);
    print_diags(&linker_diags, input_path, &source);
    if has_errors(&linker_diags) {
        anyhow::bail!("linker errors in {input_path}");
    }

    // 5. Determine the output format and emit the final binary.
    let format = crate::output::resolve_format(args, &linked.target);
    let bytes = crate::output::emit_linked(&linked, &format)?;

    // 6. Write the bytes to the output file or to stdout.
    match &args.output {
        Some(path) => std::fs::write(path, bytes)?,
        None => {
            use std::io::Write;
            std::io::stdout().write_all(&bytes)?;
        }
    }
    Ok(())
}

/// Return true when the diagnostics list contains any error-severity entry.
fn has_errors(diags: &[op_diagnostics::Diagnostic]) -> bool {
    diags.iter().any(|d| d.severity == Severity::Error)
}

/// Print each diagnostic with the source text as context.
fn print_diags(diags: &[op_diagnostics::Diagnostic], file: &str, source: &str) {
    for d in diags {
        d.print(Some(source));
    }
    let _ = file;
}
