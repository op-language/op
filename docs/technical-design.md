# Op Compiler and Cart Build Tool Technical Design

Version 1.0

This document defines the technical design of the `opc` compiler and the
`cart` build tool. The `opc` compiler compiles Op source files into machine
code for retro game consoles and home computers. The `cart` build tool manages
Op projects the same way `cargo` manages Rust projects.

This document uses the keywords **must**, **shall**, and **may** as RFC 2119
defines.

## Scope

This document defines the `opc` binary, the `cart` binary, the crate
structure, the pipeline stages, the intermediate file formats, the error
handling, the code generation, the optimizer, the linker, the file output, the
lib management system, and the `Cart.toml` format.

This document does **not** define the Op language grammar or the per-target
opcode tables. The document `language-specification.md` defines those.

## Background

The `hlakit` Python prototype established the early language syntax and the
front-end architecture. The `hlacc` project refined the language and adopted
Rust-like syntax. The `opc` compiler is the final implementation in Rust. The
`cart` tool is the build system and package manager for Op projects.

## Design goals

1. The `opc` compiler must compile correct Op source files into correct
   machine code for the selected target.
2. The `opc` compiler must reject incorrect source files with a structured
   diagnostic.
3. The `opc` compiler must be a single binary with stage flags so that each
   stage can run and be tested independently.
4. The `opc` compiler must use a linear pipeline. Each stage reads the output
   of the previous stage and writes the input of the next stage.
5. The `opc` compiler must use JSON for the intermediate file formats so that
   a developer can inspect and debug each stage.
6. The `opc` compiler must load all CPU and platform definitions from external
   libs. The compiler must not hard-code opcode tables or register names.
7. The `opc` compiler must select the target from the triplet on the command
   line or from the `Cart.toml`.
8. The `cart` tool must manage Op projects the same way `cargo` manages Rust
   projects.
9. The `cart` tool must install libs in `~/.carts/` and resolve dependencies
   from the `Cart.toml`.
10. Both tools must run on Linux, macOS, and Windows.

## Architecture overview

The Op toolchain has two binaries. The `opc` compiler compiles source files.
The `cart` build tool manages projects, resolves dependencies, and invokes
`opc`.

### opc pipeline

The `opc` compiler has four pipeline stages. Each stage has a command-line
flag that runs only that stage and writes the intermediate file. Without a
stage flag, `opc` runs all stages in-memory and writes the final ROM image.

```
source file (.op)
     |
     v
  lexer          --lex       writes .opx (JSON token stream)
     |
     v
  parser         --parse     writes .opa (JSON AST)
     |
     v
  codegen+opt    --compile   writes .opl (JSON object/link data)
     |
     v
  linker         --link      writes .opl (JSON linked sections)
     |
     v
  file output    (no flag)   writes final ROM image (.nes, .lnx, .bin)
```

### Crates

| Crate | Type | Purpose |
|-------|------|---------|
| `opc` | binary | The Op compiler. Contains the lexer, parser, code generator, optimizer, linker, and file output. |
| `cart` | binary | The Op build tool and package manager. |
| `op-common` | library | Shared types: tokens, AST nodes, diagnostics, target descriptors. |
| `op-target` | library | Target trait and registry. |
| `op-ir` | library | Intermediate representation types for the compiler and linker. |
| `op-diagnostics` | library | Error and warning reporting. |

CPU and platform libs are external Op libraries, not Rust crates. The `cart`
tool installs them in `~/.carts/`. The `opc` compiler loads them at build
time. See the Libs section.

A workspace `Cargo.toml` file at the repository root defines all Rust crates.

## opc binary

### Command-line interface

```
opc [OPTIONS] <input>

Stage flags (mutually exclusive):
  --lex           Run the lexer only. Write a .opx file.
  --parse         Run the lexer and parser. Write a .opa file.
  --compile       Run lexer, parser, codegen, optimizer. Write a .opl file.
  --link          Run all stages through linker. Write a .opl file.

Output:
  -o <path>       Output file path.
                  Default: stdout for stage flags, input name with
                  target-specific extension for full pipeline.
  --format <name> Output format for final ROM (ines, lnx, raw, hex).
                  Default: the format for the target.

Target:
  --target <triplet>  Target triplet, e.g. mos6502-nintendo-nes-ntsc.
  --cpu <name>        CPU family name (overrides triplet CPU).
  --feature <name>    Enable a feature flag.

Include paths:
  -I <path>       Add a directory to the include search path.

Optimization:
  -O <level>      Optimization level (0 = none, 1 = keyhole peephole).
                  Default: 1.

Diagnostics:
  --error-limit <n>   Stop after n errors. Default: 20.
```

### Stage flags

The four stage flags are mutually exclusive. Each flag runs the pipeline from
the beginning through the named stage and writes the intermediate file.

| Flag | Runs | Writes | Extension |
|------|------|--------|-----------|
| `--lex` | lexer | JSON token stream | `.opx` |
| `--parse` | lexer + parser | JSON AST | `.opa` |
| `--compile` | lexer + parser + codegen + optimizer | JSON object/link data | `.opl` |
| `--link` | lexer + parser + codegen + optimizer + linker | JSON linked sections | `.opl` |

Without a stage flag, `opc` runs all stages in-memory and writes the final ROM
or binary image. The output format depends on the target or the `--format`
flag.

### Intermediate file formats

All intermediate files use JSON. A developer can inspect and debug each stage
by writing the intermediate file and reading it.

#### .opx (post-lexer token stream)

```json
{
  "version": 1,
  "file": "main.op",
  "tokens": [
    { "type": "Kw_fn", "value": "fn", "line": 1, "col": 1 },
    { "type": "IDENT", "value": "main", "line": 1, "col": 4 },
    { "type": "LPAREN", "value": "(", "line": 1, "col": 8 },
    { "type": "RPAREN", "value": ")", "line": 1, "col": 9 },
    { "type": "LBRACE", "value": "{", "line": 1, "col": 11 }
  ]
}
```

See `file-formats.md` for the full `.opx` format specification and the
complete token type reference table.

#### .opa (post-parser AST)

```json
{
  "version": 1,
  "target": "mos6502-nintendo-nes-ntsc",
  "root": {
    "kind": "Module",
    "name": "game",
    "items": [
      {
        "kind": "FnDecl",
        "name": "main",
        "is_noreturn": true,
        "attributes": [
          { "path": "interrupt", "args": [ { "name": "", "value": "reset" } ] }
        ],
        "body": [
          { "kind": "AsmStmt", "opcode": "lda", "operands": [
            { "kind": "Immediate", "value": { "kind": "Number", "value": 0 } }
          ] }
        ]
      }
    ]
  }
}
```

#### .opl (post-compile or post-link object data)

Post-compile (.opl):

```json
{
  "version": 1,
  "target": "mos6502-nintendo-nes-ntsc",
  "sections": [
    {
      "name": "rom_bank0",
      "kind": "rom",
      "org": 49152,
      "bank": 0,
      "maxsize": 16384,
      "symbols": [
        { "name": "main", "offset": 0, "size": 12, "kind": "function" }
      ],
      "relocations": [
        { "offset": 5, "kind": "abs16", "symbol": "foo" }
      ],
      "data": [ 169, 0, 141, 0, 32, 96 ]
    }
  ]
}
```

Post-link (.opl after `--link`): all relocations are resolved, sections are
laid out in the final memory map, and the data array contains the final bytes.

### Output formats

| Format | Target | Description |
|--------|--------|-------------|
| `ines` | NES | iNES ROM file with 16-byte header. |
| `lnx` | Atari Lynx | .lnx ROM file with Lynx header. |
| `raw` | Any | Raw binary image with no header. |
| `hex` | Any | Intel HEX file for EEPROM burners. |

## opc pipeline stages

### Stage 1: lexer

The lexer reads the source file as UTF-8 text. The lexer splits the text into
tokens. The lexer discards whitespace and comments. The lexer records the line
and column of each token.

The lexer uses the `logos` crate to generate the tokenizer from regular
expressions.

The lexer reads the target triplet from the command line. The lexer loads the
target lib from `~/.carts/`. The lib provides the list of CPU-specific
opcode mnemonics. The lexer classifies an identifier as an opcode token if the
lib lists it.

The lexer writes the token stream as JSON (.opx).

The lexer does **not** follow `mod` declarations, `locate_bytes!` macros,
`locate_str!` macros, or `locate_fn!` macros. The lexer emits the keywords and
the arguments as tokens. The parser stage resolves modules and includes.

### Stage 2: parser

The parser reads the JSON token stream (.opx). The parser builds an AST that
the normalized LR(1) grammar defines. The parser uses the `lalrpop` crate to
generate the parser from the grammar.

The parser loads the target lib. The lib provides the list of valid opcodes,
registers, and condition keywords. The parser validates that each opcode,
register, and condition keyword is legal for the target.

The parser resolves a `mod name;` declaration by finding the named file in the
same directory as the current module. It looks for `name.op` first. If
`name.op` does not exist, it looks for `name/mod.op`. The parser does not
search sibling directories. The parser does not search an include path for
module files. The parser resolves a `use` declaration by walking the path. A
dependency lib name resolves to the lib root in `~/.carts/`. The `lib::` root
resolves to the current lib root. The `self::` root resolves to the current
module. The `super::` root resolves to the parent module. A relative name
resolves to a child module or item in the current module.

The parser evaluates `#[cfg(...)]` attributes. If the predicate is false, the
parser drops the item from the AST. The parser evaluates the cfg predicate
with the target triplet and the feature flag set.

The parser evaluates `const` expressions with the constant evaluator. The
parser stores the computed value in the AST node.

The parser writes the AST as JSON (.opa).

#### Constant evaluator

The constant evaluator runs during parsing. The evaluator computes the value
of each `const` expression and each immediate expression. The evaluator
supports the full operator set and the `lo!`, `hi!`, `nylo!`, `nyhi!`, and
`sizeof!` macros.

The evaluator reports an error if a constant expression divides by zero, if a
`sizeof!` operand is unknown, or if a `lo!` or `hi!` operand is out of range.

### Stage 3: code generation and optimization

The code generator reads the JSON AST (.opa). The code generator generates an
object file with sections, symbols, relocations, and data bytes.

#### Code generator

The code generator walks the AST. For each function, the code generator emits
a sequence of instructions. For each variable, the code generator allocates
space in the appropriate section.

The code generator loads the target lib. The lib provides the opcode
encoding table. The code generator uses the table to encode each instruction
into bytes.

For each instruction, the code generator:

1. Looks up the opcode mnemonic in the lib table.
2. Determines the addressing mode from the operand form.
3. Selects the shortest encoding if the program did not force a mode.
4. Emits the opcode byte and the operand bytes.
5. Records a relocation entry if the operand references a symbol whose address
   is not yet known.

For each control-flow construct, the code generator emits branch and label
instructions:

| Construct | Emitted code |
|-----------|-------------|
| `if (cond) { body }` | Branch-if-not-condition over the body. |
| `if (cond) { body } else { else_body }` | Branch-if-not-condition over body, jump past else_body, else_body follows. |
| `while (cond) { body }` | Label at top, branch-if-not-condition past body, jump to label at end. |
| `do { body } while (cond)` | Label at top, body, branch-if-condition to label. |
| `loop { body }` | Label at top, body, jump to label. |
| `switch (reg) { cases }` | Sequence of compare-and-branch chains. |
| `return` | Return-from-subroutine instruction. |
| `fn call()` | Jump-to-subroutine instruction. |
| `inline fn call(a, b)` | Body expanded at call site with parameter substitution. |

For each `#[rom(...)]` block, the code generator creates a ROM section. For
each `#[ram(...)]` block, the code generator creates a RAM section. For each
`#[chr(...)]` block, the code generator creates a CHR section.

For each `#[addr(...)]` variable, the code generator binds the variable to the
fixed address. For each variable without an address, the code generator
allocates space at the current section offset.

For each `#[interrupt(name)]` function, the code generator records the
interrupt vector entry. The linker writes the vector entry into the vector
table.

For each `locate_bytes!("file")` macro, the code generator reads the binary
file and emits the bytes into the current section.

For each `locate_fn!(path::name)` macro, the code generator locates the named
function in the referenced module, compiles its body, and emits the
instructions into the current ROM section at the current offset. If the
`locate_fn!` call has an `#[interrupt(...)]` attribute, the code generator
records the interrupt vector entry.

For each `#[ines(...)]` or `#[lnx(...)]` attribute, the code generator records
the header fields. The linker writes the header into the output file.

#### Addressing-mode selection

The code generator selects the addressing mode as follows:

1. If the program used an explicit mode prefix, the code generator uses that
   mode. The code generator emits a warning if the linker finds that the
   explicit mode is not the optimal mode.
2. If the operand value is known and fits in one byte and the label is in zero
   page, the code generator uses the zero-page mode.
3. Otherwise, the code generator uses the absolute mode.
4. For branch instructions, the code generator uses the relative mode. If the
   `far` keyword is present, the code generator uses an absolute jump.
5. The code generator emits a warning when the linker resolves a zero-page
   versus absolute ambiguity.

#### Optimizer

The optimizer runs after the code generator. The optimizer performs keyhole
peephole optimization. The optimizer operates on a window of consecutive
instructions within a single function body.

The optimizer implements the following transforms:

| Transform | Pattern | Result |
|-----------|---------|--------|
| Redundant load | `lda X` then `lda X` | `lda X` |
| Redundant store | `sta X` then `sta X` | `sta X` |
| Load-store-load | `lda X` then `sta Y` then `lda Y` | `lda X` then `sta Y` |
| Dead store | `sta X` then no read of X before next store to X | Remove the store. |
| Branch to next | `bra L` then `L:` | Remove the branch. |
| Branch to branch | `bra L1` then `L1: bra L2` | `bra L2` |
| Constant fold | `lda #(1 + 2)` | `lda #3` |
| Strength reduce | `lda #0` then `clc` then `adc #0` | `lda #0` |
| Stack push-pop | `pha` then `pla` | Nothing (if no side effect between) |

The optimizer respects the `volatile` keyword. The optimizer must not remove
a load or store of a volatile variable.

The optimizer runs only if the command-line optimization level is 1 or higher.

The optimizer does **not** perform block rearrangement, cross-function
dead-code elimination, jump threading, or constant propagation. A future
revision may add these.

### Stage 4: linker

The linker reads the JSON object data (.opl post-compile). The linker resolves
all relocations, lays out the sections in the final memory map, and writes the
final linked data (.opl post-link).

#### Linker steps

1. **Collect.** The linker reads all sections from the object data.
2. **Merge.** The linker merges sections that have the same name and bank. The
   linker concatenates the data and adjusts the symbol offsets.
3. **Resolve.** The linker resolves all symbol references. The linker computes
   the final address of each symbol from the section origin and the symbol
   offset.
4. **Patch.** The linker patches each relocation with the resolved address.
   The linker checks that each branch relocation is in range. If a branch is
   out of range, the linker emits an error unless the `far` keyword was
   present.
5. **Lay out.** The linker places the sections in the target memory map.
6. **Vector table.** The linker writes the interrupt vector entries into the
   vector table at the target-defined addresses.
7. **Header.** The linker writes the output file header (iNES, .lnx, or none)
   from the recorded header fields.
8. **Pad.** The linker pads each section to its maxsize with the padding byte.
9. **Write.** The linker writes the final binary image to the output file.

#### Memory map

Each target defines a memory map. The lib provides the memory map as a list
of regions. Each region has a name, a kind (rom, ram, chr), a base address, a
size, and a bank count.

#### Relocation kinds

| Kind | Size | Description |
|------|------|-------------|
| `abs8` | 1 byte | Absolute 8-bit address. |
| `abs16` | 2 bytes | Absolute 16-bit address. |
| `abs24` | 3 bytes | Absolute 24-bit address (65C816). |
| `abs32` | 4 bytes | Absolute 32-bit address (68000). |
| `branch8` | 1 byte | Relative branch offset, 8-bit signed. |
| `branch16` | 2 bytes | Relative branch offset, 16-bit signed. |
| `lo8` | 1 byte | Low byte of a 16-bit symbol address. |
| `hi8` | 1 byte | High byte of a 16-bit symbol address. |
| `bank` | 1 byte | Bank number of a symbol (Lynx). |

### Stage 5: file output

When `opc` runs without a stage flag, the file output stage reads the linked
data and writes the final ROM or binary image. The output format depends on
the target or the `--format` flag.

**iNES (NES):** 16-byte header with mapper, mirroring, battery, trainer,
fourscreen, PRG ROM size, CHR ROM size. PRG and CHR banks concatenated.

**.lnx (Atari Lynx):** .lnx header with version, name, manufacturer, rotation,
bank count, block count, block size. ROM blocks concatenated.

**raw:** Raw binary image with no header. Suits the Atari Lynx two-stage build.

**Intel HEX:** Sections as Intel HEX records. Suits EEPROM burners.

## Libs

Libs are Op libraries that provide CPU and platform definitions. The `opc`
compiler loads libs at build time. No libs are built into `opc`.

### Lib structure

A lib is a directory in `~/.carts/` with a `Cart.toml` and a `src/lib.op`
file. The `Cart.toml` declares the lib name and its dependencies.

A CPU lib provides:

1. The `cpu` module with register constants.
2. The `enum` groups for CPU status flags and condition tests.
3. The opcode encoding table.
4. The addressing mode definitions.
5. The interrupt vector definitions.

A platform lib depends on a CPU lib and adds:

1. The platform module with memory-mapped IO addresses.
2. The platform constants.
3. The standard inline macros for assignment, arithmetic, bitwise, stack, and
   memory operations.

### Lib installation

The `cart install <name>` command fetches a lib from a git-based registry and
installs it in `~/.carts/<name>/`. The registry is a git repository for now. A
future revision may define a proper registry protocol.

The `Cart.toml` `[dependencies]` section lists the libs that a project uses.
The `cart` tool resolves dependencies from `~/.carts/` before invoking `opc`.

A lib exports only its `pub` items. Private items are not visible to libs or
ROMs that depend on this lib. The `pub use` re-export form makes an imported
item visible to downstream consumers. The compiler records the `pub` flag on
each symbol in the `.opb` symbol table. The linker reads the flag to decide
whether a symbol is visible across a lib boundary. See `file-formats.md` for
the symbol table entry layout.

### Lib names

Example lib names: `mos6502`, `mos65sc02`, `ricoh2a03`,
`wdc65c816`, `m68000`, `z80`, `lr35902`, `nes`,
`lynx`, `gameboy`, `snes`, `genesis`.

## Target abstraction

The `op-target` crate defines a `Target` trait. Each lib implements the
trait.

```rust
pub trait Target {
    fn triplet(&self) -> &str;
    fn cpu(&self) -> &dyn Cpu;
    fn platform(&self) -> &dyn Platform;
    fn memory_map(&self) -> &[MemoryRegion];
    fn output_format(&self) -> OutputFormat;
}

pub trait Cpu {
    fn opcodes(&self) -> &[OpcodeDef];
    fn registers(&self) -> &[RegisterDef];
    fn conditions(&self) -> &[ConditionDef];
    fn addressing_modes(&self) -> &[AddressingModeDef];
    fn encode(&self, mnemonic: &str, mode: AddressingMode, operand: Option<u32>) -> Option<Vec<u8>>;
    fn interrupt_vectors(&self) -> &[InterruptVectorDef];
}

pub trait Platform {
    fn name(&self) -> &str;
    fn defines(&self) -> &[(String, u32)];
    fn header_attributes(&self) -> &[HeaderAttrDef];
}
```

The compiler, the parser, and the linker use the `Target` trait to query the
target. No binary hard-codes a target.

The `op-target` crate provides a registry. The registry maps a triplet string
to a `Target` constructor. The registry loads libs from `~/.carts/` at build
time.

## cart build tool

The `cart` tool manages Op projects the same way `cargo` manages Rust
projects.

### cart init

```
cart init [OPTIONS] <name>
```

Creates a new Op project with the given name. Creates a git repository, a
`Cart.toml`, a `.gitignore`, and a `src/` directory.

By default `cart init` creates a cart (binary ROM) project with
`src/cart.op`. When `--lib` is passed, it creates a lib (library) project
with `src/lib.op` instead.

Options:

| Flag | Description |
|------|-------------|
| `--lib` | Create a library (lib) project with `src/lib.op`. |
| `--target <triplet>` | Set the default target triplet in `Cart.toml`. |

### cart build

```
cart build [OPTIONS]
```

Resolves dependencies from `~/.carts/`, invokes `opc` to compile the project,
and writes the final ROM image to the output directory.

Options:

| Flag | Description |
|------|-------------|
| `--target <triplet>` | Override the target triplet. |
| `--release` | Build with optimization level 1. |
| `--debug` | Build with optimization level 0. |
| `--feature <name>` | Enable a feature flag. |
| `--format <name>` | Override the output format. |

### cart run

```
cart run [OPTIONS]
```

Builds the project and launches the ROM in the configured emulator.

The `Cart.toml` `[run]` section specifies the emulator:

```toml
[run]
emulator = "mesen"
args = ["--rom"]
```

If no emulator is configured, `cart run` prints an error.

### cart test

```
cart test [OPTIONS]
```

Runs the project's test suite. Tests are Op source files in the `tests/`
directory.

### cart check

```
cart check [OPTIONS]
```

Runs the lexer and parser without generating code. Reports errors and
warnings. Faster than `cart build`.

### cart clean

```
cart clean
```

Removes the build output directory.

### cart add

```
cart add <name>
```

Adds a lib to the `Cart.toml` `[dependencies]` section. Fetches the lib
from the registry and installs it in `~/.carts/`.

### cart doc

```
cart doc [OPTIONS]
```

Generates documentation from the doc comments in the project's Op source
files.

### cart install

```
cart install <name>
```

Fetches a lib from the registry and installs it in `~/.carts/<name>/`.
Does not modify the `Cart.toml`.

### cart update

```
cart update
```

Updates all dependencies listed in `Cart.toml` to the latest version from the
registry.

## Cart.toml

The `Cart.toml` file is the project manifest. It mirrors the `Cargo.toml`
structure.

### Full example

```toml
[package]
name = "nes-demo"
version = "0.1.0"
edition = "1"
authors = ["Dave Huseby <dave@linuxprogrammer.org>"]
license = "BSD-2-Clause"

[lib]
name = "nes-demo-lib"
path = "src/lib.op"

[[rom]]
name = "nes-demo"
path = "src/cart.op"
target = "mos6502-nintendo-nes-ntsc"

[dependencies]
mos6502 = "1.0"
nes = "1.0"

[target]
default = "mos6502-nintendo-nes-ntsc"

[features]
debug = []
undocumented = []

[run]
emulator = "mesen"
args = ["--rom"]
```

### Field reference

#### [package]

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | yes | Project name. |
| `version` | string | yes | Semantic version. |
| `edition` | string | no | Language edition. Default: "1". |
| `authors` | list of strings | no | Author names. |
| `license` | string | no | License identifier. |

#### [lib]

Defines a library (lib) target. A project may have at most one `[lib]`
section.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | yes | Lib name. |
| `path` | string | no | Root source file. Default: `src/lib.op`. |

#### [[rom]]

Defines a binary (ROM) target. A project may have multiple `[[rom]]`
sections for different targets.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | yes | Binary name. |
| `path` | string | no | Root source file. Default: `src/cart.op`. |
| `target` | string | yes | Target triplet. |

#### [dependencies]

Lists the libs that the project depends on. Each entry is a lib name and a
version specifier.

```toml
[dependencies]
mos6502 = "1.0"
nes = "1.0"
lynx = { version = "1.0", git = "https://github.com/wookie/lynx" }
```

#### [target]

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `default` | string | no | Default target triplet. |

#### [features]

Defines feature flags for conditional compilation. Each feature is a name
with an empty list of sub-features.

```toml
[features]
debug = []
undocumented = []
```

#### [run]

Configures the emulator for `cart run`.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `emulator` | string | yes | Emulator command. |
| `args` | list of strings | no | Arguments to pass to the emulator. |

## cart metadata

The `cart` tool supports user metadata in `~/.cart/` the same way `cargo`
supports `~/.cargo/`. The `~/.cart/config.toml` file stores global
configuration.

```toml
[registry]
default = "https://github.com/wookie/op-lib-registry"

[build]
target = "mos6502-nintendo-nes-ntsc"
opt-level = 1
```

## Error handling

The `op-diagnostics` crate defines the diagnostic types. A diagnostic has a
severity (error, warning, note), a file path, a line, a column, a message, and
an optional list of related spans.

Both `opc` and `cart` use the same diagnostic format. The tools print
diagnostics to stderr in the following format:

```
error[EXXX]: message text
  --> file.op:line:col
   |
   | source line text
   |       ^^^^ hint text
   |
```

The `EXXX` code is a three-digit number. The first digit names the stage:
1 = lexer, 2 = parser, 3 = codegen, 4 = linker, 5 = cart.

A tool exits with status code 1 if it emits one or more errors. A tool exits
with status code 0 if it emits only warnings or no diagnostics.

The `--error-limit n` flag tells `opc` to stop after n errors. The default is
20.

## Testing

Each crate has a `tests/` directory with integration tests. The tests use the
standard Rust test harness.

### Lexer tests

Read sample source files and check that the token stream matches an expected
JSON output. Cover all keyword forms, number literals, string literals,
comments, attributes, and target-specific opcode tokens.

### Parser tests

Read sample token streams and check that the AST matches an expected JSON
output. Cover each declaration form, control-flow construct, addressing-mode
form, conditional compilation, and error cases.

### Compiler tests

Read sample AST files and check that the object data matches an expected JSON
output. Cover opcode encoding, section layout, relocation generation,
optimizer transforms, and addressing-mode selection.

### Linker tests

Read sample object data and check that the output binary matches an expected
byte array. Cover relocation resolution, section merging, vector table
generation, header generation, padding, and out-of-range branch errors.

### End-to-end tests

Run the full `opc` pipeline on the example programs. Check that the final ROM
image matches a known-good byte array.

### cart tests

Test `cart init`, `cart build`, `cart check`, and `cart clean` on sample
projects. Test dependency resolution and lib installation.

## Build and distribution

The workspace uses Cargo. A developer builds all binaries with `cargo build`.
A developer runs all tests with `cargo test`.

The release build produces two binaries: `opc` and `cart`. A user installs
them with `cargo install` or from a distribution package.

Banks are Op libraries installed via `cart install`. They are not Rust crates
and are not built with Cargo. The `opc` compiler loads them as Op source at
build time.

## Command-line usage examples

### Full pipeline with opc

```
opc --target mos6502-nintendo-nes-ntsc -o nes.nes game.op
```

### Stage-by-stage with opc

```
opc --target mos6502-nintendo-nes-ntsc --lex -o game.opx game.op
opc --target mos6502-nintendo-nes-ntsc --parse -o game.opa game.opx
opc --target mos6502-nintendo-nes-ntsc --compile -o game.opl game.opa
opc --target mos6502-nintendo-nes-ntsc --link -o game.opl game.opl
```

### With cart

```
cart init --target mos6502-nintendo-nes-ntsc nes-demo
cd nes-demo
cart build
cart run
```

### Install a lib

```
cart install mos6502
cart install nes
```

### Atari Lynx two-stage build

Stage 1: compile the game executable as a raw binary.

```
opc --target mos65sc02-atari-lynx --format raw -o game.bin game.op
```

Stage 2: compile the ROM image that includes the raw binary.

```
opc --target mos65sc02-atari-lynx --format lnx -o game.lnx rom.op
```

## Conformance

A conforming `opc` implementation must:

1. Implement all four stage flags as this document defines.
2. Use JSON for all intermediate file formats (.opx, .opa, .opl).
3. Load all CPU and platform definitions from external libs in `~/.carts/`.
4. Implement the code generator for at least one CPU family.
5. Implement the keyhole peephole optimizer as this document defines.
6. Implement the linker for at least one output format.
7. Report errors with the structured diagnostic format.
8. Exit with status code 1 on any error.

A conforming `cart` implementation must:

1. Implement `cart init`, `cart build`, `cart run`, `cart test`, `cart check`,
   `cart clean`, `cart add`, `cart doc`, `cart install`, and `cart update`.
2. Read and write the `Cart.toml` format as this document defines.
3. Install libs in `~/.carts/`.
4. Resolve dependencies from `~/.carts/` before invoking `opc`.
5. Support the `~/.cart/config.toml` metadata file.

## Future work

1. Block rearrangement optimizer.
2. Cross-function dead-code elimination.
3. Jump threading.
4. Constant propagation.
5. A proper lib registry protocol beyond git URLs.
6. A foreign-function interface for C and assembly object files.
7. Dynamic loading of libs as shared libraries.
8. Language server protocol support for editor integration.
9. Debug information generation for emulators.
10. `cart publish` for publishing libs to the registry.