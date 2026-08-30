# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.12.0]

### Added
- `opc` codegen: a `#[locate(...)]` placement attribute. Declare it on a
  const declaration or on a fn declared inside a ROM block.
  `#[locate(addr = 0x00A8)]` pins the item at that absolute ROM address:
  the gap up to the pin fills with the pad byte, and the linker resolves
  symbols and relocations to the pinned address. With a
  `#[locate(addr = 0x00A8, file = "blob.bin")]` argument pair, the file
  bytes emit at the pinned address. A const with only a `file` argument
  (no `addr`) emits the file bytes at the current section offset. A pin
  is a placement directive: the item places even when no live code
  references it.
- `opc` codegen: consts declared inside `#[chr]` blocks now emit into
  that CHR section (previously only ROM blocks collected const data).

### Changed
- `opc` codegen: the SM83 (Game Boy) 0x0000-0x014F reservation now runs
  only when the source declares a `#[gb]` header attribute. A raw-format
  program (for example, a boot ROM) keeps full control of $0000-$014F.
- `opc` codegen: a fn body that ends with `ret`, `reti`, `jr`, or a
  conditional `jr` no longer receives a trailing implicit `RET`/`RTS`.
  This removes a stray 0xC9 byte after such fns.

### Removed
- `opc`: the `locate_bytes!` and `locate_fn!` placement macros and their
  lexer tokens. Use `#[locate(addr = 0x00A8, file = "x.bin")]` to place
  external file bytes at an address, and declare the fn inside the
  `#[rom]` block to place it. `locate_str!` remains: it includes Op
  source text and is not a placement macro.

## [0.11.0]

### Changed
- `opc` codegen: for SM83 (Game Boy) targets, reserve the first 0x150
  bytes of the bank-0 ROM section (org 0x0000) for the interrupt
  vectors and cartridge header. Code now starts at 0x0150, so the reset
  handler is no longer clobbered by the header. This eliminates the
  need for a manual `jp real_main` + nop trampoline in Game Boy
  programs.
- `opc` `emit_gb`: the `#[gb()]` annotation now initializes 0x0000-0x0104
  with 0x00 and writes the cartridge header to 0x0104-0x014F. The
  `#[interrupt(reset)]` is processed after that, writing `0x0000 = JP
  <main>` and `0x0100 = JP 0x0000` (the SM83 reset entry bounces to
  0x0000, which jumps to main). Other interrupt vectors are re-written
  after the zeroing so they are preserved.
- `opc` optimizer: fixed font loading relocations and code generation
  for struct-pointer fields used in `ld_de #data` relocations.

## [0.10.0]

### Added
- `VectorEncoding` enum in `op-ir` (`lib.rs`). The enum has four
  variants: `Pointer2` (2-byte LE address, 6502 family and 65C816),
  `Pointer4` (4-byte BE address, 68000), `JumpZ80` (3-byte JP
  instruction, Z80), and `JumpSm83` (3-byte JP instruction, SM83).
- `encoding` field on `InterruptVector` in `op-ir`. The field tells
  the linker how to encode each vector entry. Defaults to
  `Pointer2` for backward compatibility.
- Z80 interrupt vector support in `interrupt_vector_address`
  (`codegen.rs`). Supports `reset` (0x0000), `rst8` through `rst38`
  (0x0008 through 0x0038), `irq` (alias for `rst38`), and `nmi`
  (0x0066).
- 68000 interrupt vector support in `interrupt_vector_address`
  (`codegen.rs`). Supports the full exception vector table: `reset`
  (0x0000), `reset_pc` (0x0004), `bus_error` (0x0008),
  `address_error` (0x000C), `illegal` (0x0010), `zero_divide`
  (0x0014), `chk` (0x0018), `trapv` (0x001C), `privilege` (0x0020),
  `trace` (0x0024), `line_a` (0x0028), `line_f` (0x002C),
  `spurious` (0x0060), `level1` through `level7` (0x0064 through
  0x007C), and `trap0` through `trap15` (0x0080 through 0x00BC).
- `vector_encoding_for` function in `codegen.rs`. Returns the
  `VectorEncoding` for a given CPU family.
- `add_interrupt_vector` helper method on the codegen struct. Sets
  the encoding per CPU family and handles the 68000 reset vector
  specially (emits SSP at 0x0000 with target `_stack_top` and PC at
  0x0004 with the handler target).
- Linker `_stack_top` resolution. The linker computes the stack top
  from the first RAM section (org + maxsize) when the `_stack_top`
  symbol is not in the symbol table.
- New linker tests for Z80 reset and NMI vectors, SM83 vblank
  vector, 68000 reset vector (SSP + PC), and 68000 level-2
  autovector.
- New codegen tests for `interrupt_vector_address` on all CPU
  families and for `vector_encoding_for`.

### Changed
- SM83 vector encoding changed from `Pointer2` (handler address at
  vector slot) to `JumpSm83` (3-byte JP instruction at vector slot).
  The handler function can now live anywhere in ROM. The linker
  emits `0xC3` + 2-byte LE address at the vector address.
- The linker `write_vector_tables` method now dispatches on the
  `VectorEncoding` to write the correct byte format per CPU family.
- Integration test data moved from `examples/` to
  `crates/opc/tests/data/`. The `include_str!` and `include_bytes!`
  paths in `integration.rs` now reference `data/nes.op` and
  `data/font.chr` instead of `../../../examples/nes.op` and
  `../../../examples/font.chr`. The `examples/` directory is deleted.
- `ENCODING_VL65NC02` encoding table in `encoding.rs`. The table is an
  exact copy of `ENCODING_65SC02`. The VL65NC02 is a 65SC02 core.
- `ENCODING_SM83` encoding table in `encoding.rs`. The table holds
  accurate SM83 opcodes for the Sharp CPU used in the Game Boy.
- `vl65NC02` and `sm83` arms in `get_encoding_table` and
  `get_full_encoding_table`.
- `vl65NC02` arm in `interrupt_vector_address` with the 6502-family
  vector addresses.
- `sm83` arm in `interrupt_vector_address` with fixed Game Boy vector
  addresses: vblank at 0x0040, lcdc at 0x0048, timer at 0x0050, serial
  at 0x0058, joypad at 0x0060.
- Known limitation note for the `all`, `any`, and `not` cfg
  combinators in `language-specification.md`.
- Updated `crates/opc/tests/integration.rs` to reference
  `mos6502/mod.op` instead of the deleted `mos6502.op` flat file.
- Updated `crates/opc/tests/codegen.rs` `std_resolves_cpu_glob` test
  to check for inline fns from the 6502 macro re-export.

## [0.9.0]

### Added
- `rp2A03` and `rp2A07` CPU arms in `get_encoding_table` and
  `get_full_encoding_table` (`encoding.rs`). Both reuse `ENCODING_6502`.
  The Ricoh chips execute the full 6502 opcode set. CLD and SED run
  as no-ops.
- `rp2A03` and `rp2A07` arms in `interrupt_vector_address`
  (`codegen.rs`). Both map reset to 0xFFFC, nmi to 0xFFFA, and irq to
  0xFFFE.
- New codegen tests verify the encoding-table lookup and the
  interrupt-vector resolution for the new CPUs.

### Changed
- The `--target` help example shows
  `rp2A03-nintendo-nes-ntsc`.
- All compiler tests use `rp2A03-nintendo-nes-ntsc` as the fixture
  target. The `mos6502` CPU stays valid for the other targets.
- The `examples/nes.op` header comment references
  `rp2A03-nintendo-nes-ntsc`.

### Removed
- Dead `ricoh2a03` and `ricoh2a07` arms in
  `interrupt_vector_address`.

## [0.8.0]

### Added
- The `--output-stages` command-line flag. This flag writes the `.opx`,
  `.opa`, `.opl`, and `.linked.opl` intermediate files and the final
  binary when the full pipeline runs.
- Intermediate-file stage chaining. The `--parse` flag reads a `.opx`
  file. The `--compile` flag reads a `.opa` file. The `--link` flag reads
  a `.opl` file. Each stage writes the intermediate envelope for the next
  stage.

### Fixed
- The peephole optimizer no longer runs on CHR or RAM sections. CHR
  pattern data and RAM data stay intact at the default optimization
  level.
- The peephole optimizer is now relocation-aware. Stores to different
  symbols with identical placeholder operand bytes are no longer treated
  as writes to the same address. The `dead_store` and `load_store_load`
  transforms compare relocation indices, not raw bytes.
- The peephole optimizer marks NES PPU and APU memory-mapped register
  accesses ($2000-$2007, $4000-$4017) as volatile. The dead-store and
  redundant-store transforms no longer remove required hardware
  register writes.
- The parser now treats a function call on a new line as a separate
  statement. It does not fold the call into the operands of the assembly
  instruction on the previous line.
- The dead-code analyzer now records a function call in an expression
  position as a use. It no longer reports a false warning for inline
  functions that other inline functions call.
- The code generator now emits absolute addresses for `JMP` targets in
  `loop`, `while`, and `if` else-blocks. Previously it emitted
  section-relative offsets, which jumped to the wrong address.
- The code generator now encodes indirect indexed addressing (`lda
  (ptr), y`) as `IndirectY` (`B1`) and indirect indexed with X as
  `IndirectX` (`A1`). Previously it encoded these as `AbsoluteY`/`AbsoluteX`.
- The code generator now honors the `not` modifier in `do-while`
  conditions. The branch direction is correctly inverted.
- The code generator now appends an implicit `RTS` to functions that do
  not end with an explicit `return`, `RTS`, `RTI`, `JMP`, or `BRA`.
- Empty interrupt handler bodies no longer overlap with following data.
  The `nes.op` example adds `RTI` to the `nmi` and `irq` handlers.
- The `nes.op` example and the NES std-lib macros now use immediate mode
  (`#` prefix) for constant operands in `assign`, `vram_write`,
  `system_initialize`, `vram_clear_address`, `vram_init`, `wait_for`,
  and `pal_animate`.

### Tests
- 226 tests pass across all test binaries (39, 49, 20, 43, 17, 58).
- New `full_pipeline_std_nes_game` assertions verify the CHR section
  data, the CHR data length, and the iNES header bytes at optimization
  level 1.
- New `parser_inline_call_new_line` test verifies the line-boundary fix.
- New `stage_chaining` test verifies the `--lex`, `--parse`,
  `--compile`, and `--link` flags chain through intermediate files.
- New `output_stages_flag` test verifies the `--output-stages` flag
  writes all intermediate files and a byte-identical final binary.
- New `optimizer_changes_rom_but_not_chr` test verifies the optimizer
  folds ROM data and leaves CHR data unchanged.

## [0.7.0]

### Added
- Std-lib module resolution. The compiler resolves `use std::cpu::*` and
  `use std::machine::*` declarations at compile time. It searches for the
  std crate root in `--include` directories, the `OP_STD_PATH` environment
  variable, and `~/.carts/std/src`.
- Module cache. Parsed std modules are cached by absolute file path so
  repeated imports do not reparse.
- Inline fn parameter substitution. Calls to `inline fn` declarations are
  expanded at the call site with parameter substitution, including nested
  inline calls and selector path substitution.
- Enum variant constant resolution. Explicit and implicit enum variant
  values are evaluated at compile time. Glob imports of enums bind variant
  names bare.
- Multi-pass const resolution. The collect pass gathers constants, enums,
  and imports before any fn body is compiled, so fn bodies can reference
  declarations that appear later in the file.
- `lo!`/`hi!` symbol relocations. `lda #lo!(HELLO)` emits a `Lo8`
  relocation; `lda #hi!(HELLO)` emits a `Hi8` relocation. The linker
  patches the byte with the low or high byte of the symbol's address.
- `len!` and `sizeof!` compile-time macros. `len!(HELLO)` returns the
  element count of an array type. `sizeof!(ptr)` returns the byte size of
  a type. Both resolve at compile time.
- Dependency-tree function and data placement. The compiler builds a
  dependency tree from interrupt-attribute fns, `locate_fn!` pins, and
  in-block fns. It places reachable non-inline fns and top-level consts in
  the first referrer's ROM block, duplicates them per bank, and places
  top-level vars in the first `#[ram]` block.
- `jsr` calls for non-inline fns. A call to a `fn` (not `inline fn`) emits
  a `jsr` instruction with an `Abs16` relocation instead of inlining the
  body.
- Dead-code warnings. The compiler warns about unreachable fns, unused
  inline fns, unreferenced top-level consts and vars, and unreferenced
  top-level enums.
- Relocation addends. `Relocation` entries carry an `addend` field so that
  `sta pstr + 1` correctly patches the high byte at the next address.
- Compile error for unresolvable immediates. An immediate or address
  operand that is neither a constant nor a symbol produces error 305
  instead of a silent zero byte.

### Tests
- 217 tests pass across all test binaries.

### Fixed
- `self` token bug. The lexer maps `Kw_self_` to the token string
  `Kw_self`, which the parser's `check("Kw_self")` matches. No code change
  was needed; the bug was identified as a false alarm in the plan.

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