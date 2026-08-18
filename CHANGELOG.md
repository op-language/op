# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.6.0]

### Added
- Complete linker implementation for the `opc` binary. The linker
  resolves all relocations, merges sections, writes interrupt vector
  tables, records header fields, and pads sections to their maxsize.
- Complete file output stage (Stage 5) for the `opc` binary. The output
  stage writes the final binary image in the format the target requires:
  iNES (NES), .lnx (Lynx), raw (all targets), Intel HEX (EEPROM), SEGA
  (Genesis), SNES, Game Boy, SMS, and Atari 7800.
- Full in-memory pipeline in `cli::run()`. When no stage flag is set,
  `opc` runs lex, parse, codegen, optimize, link, and emit in sequence
  and writes the final binary.
- Interrupt vector recording in the codegen. The codegen tracks
  #[interrupt(name)] attributes and records vector entries for the linker.
- Header field recording in the codegen. The codegen records #[ines(...)]
  and #[lnx(...)] attribute fields for the file output stage.
- `InterruptVector` and `HeaderFields` types in `op-ir`.
- `docs/supported-emulators.md` documenting the best Linux emulator for
  each supported system, ROM formats, install instructions, and
  debug-target configuration.
- Linker unit tests in `crates/opc/tests/linker.rs`.
- Extended integration tests with full lexer+parser+codegen+optimizer+
  linker+output pipeline tests.
- `examples/font.chr` copied so `nes-game.op` compiles end-to-end.

### Changed
- The `opc` linker no longer returns an empty `ObjectFile`. The new
  implementation resolves relocations and produces the final linked data.
- The `opc` file output stage no longer returns empty bytes. The new
  implementation writes the final binary image.
- The `opc` codegen now records interrupt vectors and header fields from
  attributes.
- The `docs/file-formats.md` post-link section now describes the resolved
  relocations, vector table, and padding.

## [0.5.0]

### Added
- Complete codegen implementation for the `opc` binary. The codegen walks
  the AST and emits an `ObjectFile` with sections, symbols, relocations,
  and data bytes. It handles all 6 CPU families (6502, 65SC02, 65C816,
  68000, Z80, LR35902) with hard-coded encoding tables.
- Keyhole peephole optimizer with all 9 transforms: redundant load,
  redundant store, load-store-load, dead store, branch-to-next,
  branch-to-branch, constant fold, strength reduce, stack push-pop. The
  optimizer respects volatile variables and runs when opt_level >= 1.
- `.opl` file format specification in `docs/file-formats.md` with the
  complete section, symbol, and relocation field reference.
- Codegen unit tests in `crates/opc/tests/codegen.rs`.
- Extended integration tests in `crates/opc/tests/integration.rs` with
  lexer+parser+codegen tests.

### Changed
- The `opc` codegen no longer returns an empty `ObjectFile`. The new
  implementation walks the AST and encodes all instructions.
- The `docs/file-formats.md` scope statement now includes the `.opl`
  format.

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