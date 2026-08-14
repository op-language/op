# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
  formats, bank management, `Cart.toml` format, and the `cart` build tool.
- Editor syntax and filetype detection support under `editor/`.
- Example Op source files under `examples/`.

### Changed
- Removed the `cart` binary crate from the workspace. The `cart` build tool
  now lives in a standalone repository.