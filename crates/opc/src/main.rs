//! The `opc` Op compiler binary.
//!
//! `opc` compiles Op source files into machine code for retro game consoles
//! and home computers. It uses a linear pipeline of four stages: lexer,
//! parser, code generation and optimization, and linker. Each stage has a
//! command-line flag that runs only that stage and writes an intermediate
//! JSON file. Without a stage flag, `opc` runs all stages in-memory and writes
//! the final ROM image.
//!
//! See `docs/technical-design.md` and `docs/language-specification.md` for the
//! full specification.

#![allow(dead_code)]

mod cli;
mod codegen;
mod lexer;
mod linker;
mod output;
mod parser;

fn main() -> anyhow::Result<()> {
    cli::run()
}
