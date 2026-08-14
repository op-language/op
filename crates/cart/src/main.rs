//! The `cart` build tool and package manager binary.
//!
//! `cart` manages Op projects the same way `cargo` manages Rust projects. It
//! reads and writes the `Cart.toml` manifest, resolves dependencies from
//! `~/.carts/`, invokes `opc` to compile projects, and installs banks from a
//! git-based registry.
//!
//! See `docs/technical-design.md` for the full specification.

#![allow(dead_code)]

mod cli;
mod manifest;

fn main() -> anyhow::Result<()> {
    cli::run()
}