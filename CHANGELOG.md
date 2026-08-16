# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0]

### Added
- Rust-style `mod`/`use` rules (pragmatic subset): `pub mod`, `pub use`,
  `as` aliases, glob imports, nested group imports, and `lib`/`self`/`super`
  path roots.
- Reserved `lib`, `self`, `super` as keywords.
- `is_pub` field on the `op-ir` `Symbol` type for export visibility.
- `editor/install.sh` to install all editor artifacts (tree-sitter parser,
  query files, ftdetect, regex syntax) in one command.

### Changed
- `mod name;` now resolves Rust-style: `name.op` or `name/mod.op` in the
  same directory. No include-path search for module files.
- The `.opb` format version bumped from `1` to `2`. The symbol table
  entry `reserved` field (offset 16) is now `flags` (bit 0 = `pub`).
- The tree-sitter grammar, the query files, and the regex syntax file
  now parse and highlight the new `use`/`mod` forms and the new keywords.
- The language specification, the technical design docs, and the
  `.opb` file-format spec document the new resolution, visibility, and
  format rules.

### Breaking
- `lib`, `self`, `super` are now reserved keywords. Code that uses these
  words as identifiers must rename them.
- The `Item::ModDecl` and `Item::UseDecl` AST node shapes changed.
  `.opa` files from older `opc` versions are not compatible.
- The `.opb` format version is `2`. A linker that expects version `1`
  must read the new `flags` field.

## [0.1.0]

### Added
- Initial `op` workspace scaffold with five crates.
- `op-common` library with shared types: `Token`, `TokenStream`,
  `AstFile`, `Module`, `Item`, `Attribute`, `TargetTriplet`, `TripletError`,
  and the `Envelope` trait with `to_json` / `from_json` helpers.
- `op-target` library with the `Target`, `Cpu`, and `Platform` traits,
  `MemoryRegion`, `OutputFormat`, and the `Registry` for target lookup.
- `op-ir` library with intermediate representation types: `ObjectFile`,
  `Section`, `Symbol`, `Relocation`, and `RelocKind`.
- `op-diagnostics` library with `Diagnostic`, `Severity`, and `Diagnostics`
  for structured error and warning reporting.
- `opc` binary with the full CLI interface: `--lex`, `--parse`, `--compile`,
  `--link` stage flags, `--target`, `--cpu`, `--feature`, `-I`, `-O`,
  `--error-limit`, `-o`, and `--format` options.
- `opc` pipeline stage scaffolding: lexer, parser, codegen, linker, and file
  output modules.
- `docs/language-specification.md` defining the Op language grammar, type
  system, declaration forms, control-flow constructs, addressing modes, cfg
  predicates, and per-target tables.
- `docs/technical-design.md` defining the `opc` pipeline, intermediate file
  formats, lib management, `Cart.toml` format, and the `cart` build tool.
- Editor syntax and filetype detection support under `editor/`.
- Example Op source files under `examples/`.

### Changed
- Removed the `cart` binary crate from the workspace. The `cart` build tool
  now lives in a standalone repository.