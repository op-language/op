# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.0]

### Added
- Complete parser implementation for the `opc` binary. The parser builds
  a full AST from the lexer token stream using a hand-written recursive-
  descent parser. It handles all module-level items, function body
  statements, expressions, operands, control-flow constructs, attributes,
  and labels.
- `#[cfg(...)]` attribute evaluation in the parser. The parser evaluates
  cfg predicates against the target triplet and feature flags and drops
  items that do not match.
- Const expression evaluation in the parser. The evaluator supports the
  full operator set and the `lo!`, `hi!`, `nylo!`, `nyhi!`, and `sizeof!`
  macros.
- `mod` file resolution in the parser. The parser finds `name.op` or
  `name/mod.op` in the same directory, lexes and parses the sub-module,
  and builds a module tree.
- Expanded AST types in `op-common` to represent all grammar nodes:
  function body statements, expressions, operands, conditions, types,
  fields, enum variants, and block attributes.
- `.opa` file format specification in `docs/file-formats.md` with the
  complete AST node type reference.
- Parser unit tests in `crates/opc/tests/parser.rs`.
- Lexer+parser integration tests in `crates/opc/tests/integration.rs`.

### Changed
- The `opc` parser no longer returns an empty `Module`. The new
  implementation builds a full AST.
- The `docs/file-formats.md` scope statement now includes the `.opa`
  format.

### Breaking
- The AST node types in `op-common` changed shape. Existing `.opa`
  files from older `opc` versions are not compatible.

## [0.3.0]

### Added
- Complete lexer implementation for the `opc` binary. The lexer tokenizes
  all Op source constructs: keywords, primitive types, operators,
  punctuation, number literals (decimal, binary, hex), string literals
  with escape sequences, labels (definition and reference), attributes,
  opcodes (all CPU families), condition keywords, condition modifiers,
  addressing-mode prefixes, compile-time macros, and include macros.
- `TokenType` enum in `op-common` with a variant for every token type
  and an `as_str()` method that returns the token type name string.
- `.opx` file format specification in `docs/file-formats.md` with the
  complete token type reference table.
- Comprehensive lexer unit tests in `crates/opc/tests/lexer.rs`.
- `opc` library crate (`src/lib.rs`) so integration tests can import
  the lexer module directly.

### Changed
- The `opc` lexer no longer uses the placeholder `split_whitespace()`
  tokenizer. The new implementation is a character-by-character scanner
  with correct line and column tracking.
- The `docs/file-formats.md` scope statement now includes the `.opx`
  format.

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