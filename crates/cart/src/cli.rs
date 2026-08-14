//! Command-line interface for `cart`.
//!
//! Implements the subcommands defined in the technical design section "cart
//! build tool": `init`, `build`, `run`, `test`, `check`, `clean`, `add`,
//! `doc`, `install`, and `update`.

use anyhow::Result;
use clap::{Parser, Subcommand};

/// The Op build tool and package manager.
#[derive(Debug, Parser)]
#[command(name = "cart", version, about, long_about = None)]
pub struct CartArgs {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create a new Op project.
    Init {
        name: String,
        /// Create a library (bank) project with src/bank.op.
        #[arg(long)]
        bank: bool,
        /// Set the default target triplet in Cart.toml.
        #[arg(long)]
        target: Option<String>,
    },
    /// Build the project.
    Build {
        /// Override the target triplet.
        #[arg(long)]
        target: Option<String>,
        /// Build with optimization level 1.
        #[arg(long)]
        release: bool,
        /// Build with optimization level 0.
        #[arg(long)]
        debug: bool,
        /// Enable a feature flag.
        #[arg(long = "feature")]
        features: Vec<String>,
        /// Override the output format.
        #[arg(long)]
        format: Option<String>,
    },
    /// Build the project and launch the ROM in the configured emulator.
    Run {
        #[arg(long)]
        target: Option<String>,
        #[arg(long)]
        release: bool,
    },
    /// Run the project's test suite.
    Test {
        #[arg(long)]
        target: Option<String>,
    },
    /// Run the lexer and parser without generating code.
    Check {
        #[arg(long)]
        target: Option<String>,
    },
    /// Remove the build output directory.
    Clean,
    /// Add a bank to the Cart.toml dependencies.
    Add { bank: String },
    /// Generate documentation from doc comments.
    Doc,
    /// Install a bank in ~/.carts/.
    Install { bank: String },
    /// Update all dependencies to the latest version.
    Update,
}

/// Entry point for the `cart` CLI.
pub fn run() -> Result<()> {
    let args = CartArgs::parse();
    match args.command {
        Command::Init { name, bank, target } => init(&name, bank, target),
        Command::Build {
            target,
            release,
            debug,
            features,
            format,
        } => build(target, release, debug, features, format),
        Command::Run { target, release } => run_cmd(target, release),
        Command::Test { target } => test(target),
        Command::Check { target } => check(target),
        Command::Clean => clean(),
        Command::Add { bank } => add(&bank),
        Command::Doc => doc(),
        Command::Install { bank } => install(&bank),
        Command::Update => update(),
    }
}

fn init(name: &str, _bank: bool, _target: Option<String>) -> Result<()> {
    eprintln!("cart init: {name} (not yet implemented)");
    Ok(())
}

fn build(
    _target: Option<String>,
    _release: bool,
    _debug: bool,
    _features: Vec<String>,
    _format: Option<String>,
) -> Result<()> {
    eprintln!("cart build (not yet implemented)");
    Ok(())
}

fn run_cmd(_target: Option<String>, _release: bool) -> Result<()> {
    eprintln!("cart run (not yet implemented)");
    Ok(())
}

fn test(_target: Option<String>) -> Result<()> {
    eprintln!("cart test (not yet implemented)");
    Ok(())
}

fn check(_target: Option<String>) -> Result<()> {
    eprintln!("cart check (not yet implemented)");
    Ok(())
}

fn clean() -> Result<()> {
    eprintln!("cart clean (not yet implemented)");
    Ok(())
}

fn add(bank: &str) -> Result<()> {
    eprintln!("cart add: {bank} (not yet implemented)");
    Ok(())
}

fn doc() -> Result<()> {
    eprintln!("cart doc (not yet implemented)");
    Ok(())
}

fn install(bank: &str) -> Result<()> {
    eprintln!("cart install: {bank} (not yet implemented)");
    Ok(())
}

fn update() -> Result<()> {
    eprintln!("cart update (not yet implemented)");
    Ok(())
}