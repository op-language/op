//! The `opc` Op compiler library.
//!
//! This crate provides the lexer, parser, code generator, linker, and
//! file output stages of the Op compiler. The binary entry point is in
//! `main.rs`. The library exposes the pipeline stages so that
//! integration tests can call them directly.

#![allow(dead_code)]

pub mod cli;
pub mod codegen;
pub mod encoding;
pub mod lexer;
pub mod linker;
pub mod optimizer;
pub mod output;
pub mod parser;
