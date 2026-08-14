# Op Language Specification

Version 1.0

This document specifies the Op programming language. The `opc` compiler
parses Op source files and compiles them into machine code for retro game
consoles and home computers. The `cart` build tool manages Op projects the same
way `cargo` manages Rust projects.

The Op language derives from NESHLA by Brian Provinciano and from the HLAKit
prototype by David Huseby. This specification normalizes the grammar so that
an LR(1) parser can read it without ambiguity.

This document uses the keywords **must**, **shall**, and **may** as RFC 2119
defines.

## Scope

This document defines the lexing rules, the grammar, the type system, the
declaration forms, the control-flow constructs, the assembly operand forms,
the target-extension mechanism, and the per-target normative tables.

This document does **not** define the `opc` binary interface, the `cart` build
tool, the intermediate file formats, or the linker output formats. The document
`technical-design.md` defines those.

## Normative references

- ASD-STE100 Simplified Technical English
- Rust Programming Language (syntax for attributes, modules, types, constants)
- MOS 6502 Instruction Set Reference
- MOS 65SC02 Instruction Set Reference
- Ricoh 2A03 / 2A07 Instruction Set Reference
- WDC 65C816 Instruction Set Reference
- Motorola 68000 Instruction Set Reference
- Zilog Z80 Instruction Set Reference
- Sharp LR35902 Instruction Set Reference

## Terms and definitions

**target**
The combination of CPU, manufacturer, machine, and variant. The target tells
the compiler which opcodes, registers, conditionals, and memory model apply.

**triplet**
The target identifier string in the form `cpu-manufacturer-machine-variant`.
Example: `mos6502-nintendo-nes-ntsc`.

**CPU family**
A group of CPU cores that share a base instruction set. Example: the MOS 6502
family includes the 6502, 65SC02, Ricoh 2A03, and Ricoh 2A07.

**platform**
The machine and variant portion of a triplet. Example: `nintendo-nes-ntsc`.

**bank**
A library of Op code. A bank provides the CPU module, the register
definitions, the memory-mapped IO addresses, the platform constants, and the
standard inline macros for one target. Banks are installed in `~/.carts/` by
the `cart` tool. A bank is the Op equivalent of a Rust crate.

**selector**
A path expression that names a field, a constant, or an offset. Selectors use
the `::` operator for module and enum access and the `.` operator for struct
field access.

**immediate**
A constant value that the assembler encodes directly into the instruction
operand bytes. An immediate operand begins with the `#` prefix.

## Lexical structure

### Character set

Source files use UTF-8 encoding. The parser reads ASCII text. Bytes outside
the ASCII range may appear in string literals and comments only.

### Whitespace

Space, tab, carriage return, and line feed are whitespace. The lexer discards
whitespace between tokens. Whitespace separates tokens but does not change
token meaning.

### Newlines

A newline ends a line. The lexer counts newlines for diagnostic line numbers.
Newlines do not end statements. Statements end at the next token that the
grammar does not accept as a continuation.

### Comments

A line comment begins with `//` and ends at the next newline. The lexer
discards the comment text.

A block comment begins with `/*` and ends with `*/`. Block comments may span
multiple lines. Block comments do not nest.

A doc comment begins with `///` and ends at the next newline. A doc comment
attaches documentation to the declaration that follows it.

A module doc comment begins with `//!` and ends at the next newline. A module
doc comment documents the module that contains it.

### Identifiers

An identifier begins with a letter or underscore. The remaining characters are
letters, digits, or underscores. Identifiers are case sensitive.

```
IDENTIFIER ::= [a-zA-Z_][a-zA-Z0-9_]*
```

### Keywords

The language reserves the following keywords. A program must not use a keyword
as an identifier.

| Keyword | Purpose |
|---------|---------|
| `fn` | Declare a function |
| `inline` | Declare an inline macro function |
| `noreturn` | Mark a function that does not return (before `fn`) |
| `return` | Exit a function early |
| `volatile` | Mark a variable as volatile (do not optimize away reads or writes) |
| `struct` | Declare a struct type |
| `type` | Declare a type alias |
| `enum` | Declare an enum with discriminants |
| `const` | Declare a compile-time constant |
| `mod` | Declare a module |
| `use` | Import a module path |
| `pub` | Mark a module item as public |
| `if` | Begin a conditional block |
| `else` | Begin the alternate branch of an if block |
| `while` | Begin a while loop |
| `do` | Begin a do-while loop |
| `loop` | Begin an endless loop |
| `switch` | Begin a switch block |
| `case` | Begin a case block inside a switch |
| `default` | Begin the default block inside a switch |
| `near` | Force a short branch |
| `far` | Force a long branch |
| `as` | Cast or alias in certain contexts |
| `true` | Boolean true literal |
| `false` | Boolean false literal |

### Number literals

| Form | Syntax | Example | Value |
|------|--------|---------|-------|
| Decimal | `[1-9][0-9]*` or `0` | `123` | 123 |
| Binary | `%[01]+` | `%11110000` | 240 |
| C-style hex | `0x[0-9a-fA-F]+` | `0xFF` | 255 |

The language does **not** support the dollar-sign hex form or the kilo (`K`)
form. A program that uses `$FF` or `32K` is in error.

### String literals

A string literal begins and ends with a double quote. The lexer interprets the
following escape sequences inside a string.

| Escape | Value | Byte |
|--------|-------|------|
| `\n` | Line feed | 0x0A |
| `\r` | Carriage return | 0x0D |
| `\t` | Horizontal tab | 0x09 |
| `\0` | Null | 0x00 |
| `\a` | Bell | 0x07 |
| `\\` | Backslash | 0x5C |
| `\"` | Double quote | 0x22 |

Any other escape sequence is an error. The lexer reports the invalid escape
and stops.

```
STRING ::= \"(\\.|[^"])*\"
```

### Operators and punctuation

| Token | Meaning |
|-------|---------|
| `#` | Immediate-operand prefix |
| `::` | Module or enum path separator |
| `.` | Struct field access |
| `+` | Add / unary positive / offset |
| `-` | Subtract / unary negative / offset |
| `*` | Multiply |
| `/` | Divide |
| `%` | Modulo (in expressions) or binary prefix (in number literals) |
| `~` | Binary inverse |
| `!` | Logical not / macro call suffix |
| `&` | Binary and |
| `^` | Binary exclusive or |
| `\|` | Binary or |
| `>>` | Shift right |
| `<<` | Shift left |
| `>` | Greater than |
| `<` | Less than |
| `>=` | Greater than or equal |
| `<=` | Less than or equal |
| `==` | Equal |
| `!=` | Not equal |
| `=` | Assignment in declarations |
| `(` `)` | Grouping, call, operand |
| `{` `}` | Block, struct body, initializer |
| `[` `]` | Array dimension |
| `:` | Type annotation, address binding |
| `,` | Separator in lists |
| `;` | Statement terminator (optional in most contexts) |

### Attributes

An attribute begins with `#[` and ends with `]`. The attribute body contains a
name and an optional parenthesized argument list. Attributes attach to
declarations, blocks, or modules.

```
ATTRIBUTE ::= '#' '[' ATTR_PATH ATTR_ARGS? ']'
ATTR_PATH ::= IDENTIFIER ('::' IDENTIFIER)*
ATTR_ARGS  ::= '(' ATTR_ARG (',' ATTR_ARG)* ')'
ATTR_ARG   ::= IDENTIFIER ('=' LITERAL)?
```

The language defines the following attribute groups.

| Attribute | Applies to | Purpose |
|-----------|-----------|---------|
| `#[cfg(...)]` | Any item | Conditional compilation |
| `#[interrupt(name)]` | `fn` | Mark a function as an interrupt handler |
| `#[addr(value)]` | variable | Bind a variable to a fixed memory address |
| `#[rom(org = ..., bank = ..., maxsize = ...)]` | Block | Begin a ROM block |
| `#[ram(org = ..., maxsize = ...)]` | Block | Begin a RAM block |
| `#[chr(bank = ...)]` | Block | Begin a CHR data block (NES) |
| `#[align(value)]` | Item or block | Set alignment |
| `#[setpad(value)]` | Module | Set the padding byte value |
| `#[ines(...)]` | Module | Set NES iNES header fields |
| `#[lnx(...)]` | Module | Set Atari Lynx .lnx header fields |
| `#[loader]` | `fn` | Mark a function as the encrypted first-stage loader (Lynx) |

### Compile-time macros

The following macros evaluate at compile time. A program may call them inside
`const` expressions and inside immediate operands. They use Rust-style macro
call syntax with the trailing `!`.

| Macro | Result |
|-------|--------|
| `lo!(x)` | Low byte of a word, or low word of a dword |
| `hi!(x)` | High byte of a word, or high word of a dword |
| `nylo!(x)` | Low nibble of a byte |
| `nyhi!(x)` | High nibble of a byte |
| `sizeof!(x)` | Byte size of a variable, type, function, or file |

## Type system

### Primitive types

The language uses Rust primitive integer types. The size of each type is fixed
and does not depend on the target.

| Type | Size | Range |
|------|------|-------|
| `u8` | 1 byte | 0 to 255 |
| `i8` | 1 byte | -128 to 127 |
| `u16` | 2 bytes | 0 to 65535 |
| `i16` | 2 bytes | -32768 to 32767 |
| `u32` | 4 bytes | 0 to 4294967295 |
| `i32` | 4 bytes | -2147483648 to 2147483647 |
| `bool` | 1 byte | 0 or 1 |
| `pointer` | Target-defined | See below |

A target that has a 16-bit address bus uses a 2-byte `pointer`. A target that
has a 24-bit address bus uses a 3-byte `pointer`. A target that has a 32-bit
address bus uses a 4-byte `pointer`.

A target may restrict which types a program may use. A program that uses a type
that the target does not support is in error. Example: the MOS 6502 target does
not support `u32` or `i32` in native operations.

### Struct types

A struct type groups named fields. Each field has a type. A struct may contain
fields of any primitive type, any previously declared struct type, or any array
of those types.

```
struct POINT {
    x: u16,
    y: u16,
}
```

A struct field may be an array.

```
struct PLAYER {
    x: u8,
    y: u8,
    name: [u8; 16],
}
```

### Type aliases

A `type` declaration creates a name for another type.

```
type COORDS = POINT;
type NAME = [u8; 16];
```

### Enum types

An `enum` declaration defines a group of named constants with explicit
discriminant values. The enum defines a namespace. A program accesses the
constants with the `::` operator.

```
enum PPU {
    CNT0 = 0x2000,
    CNT1 = 0x2001,
    STATUS = 0x2002,
}
```

A program references a constant as `PPU::CNT0`.

The CPU and platform banks define the enum groups for registers and
memory-mapped IO. The compiler does not hard-code these names.

### Pointer type

The `pointer` type stores a memory address. The size depends on the target. A
program may dereference a pointer with the indirection syntax that the target
assembly defines.

## Declarations

### const declarations

A `const` declaration binds a name to a compile-time value. The value may be a
literal or a constant expression. The compiler and the optimizer substitute
the name with the value at every use.

```
const NAME_TABLE_0_ADDRESS: u16 = 0x2000;
const SCREEN_WIDTH: u8 = 256;
const SPRITE_COUNT: u8 = 64;
```

A const expression may use the full operator set and the compile-time macros.

```
const PAL_SIZE: u8 = sizeof!(PALETTE);
const EXE_SIZE: u8 = lo!(sizeof!("game.bin"));
```

### Variable declarations

A variable declaration allocates space in memory. A variable declared inside
a `#[ram(...)]` block persists for the life of the program. All variables in
RAM are mutable. The language does not restrict reads or writes of RAM
variables.

```
counter: u8 = 0;
ppu_ctl0: u8 = 0;
palcol: u8 = 0;
```

The `#[addr(value)]` attribute binds a variable to a fixed memory address.

```
#[addr(0x2000)]
PPU_CNT0: u8;

#[addr(0x4014)]
SPR_DMA: u8;
```

The `volatile` keyword marks a variable that the compiler must not optimize
away. The compiler must not remove or reorder reads or writes of a volatile
variable. A variable that both an interrupt handler and regular code reference
must be declared `volatile`.

```
volatile INPUT_FLAGS: u8 = 0;
```

The following example shows `volatile` and `#[addr(...)]` together in a RAM
block. Memory-mapped hardware registers use `#[addr(...)]` to bind to a fixed
address. A variable that an interrupt handler and the main loop both reference
uses `volatile`.

```
#[ram(org = 0x0000, maxsize = 0x100)] {
    // volatile variable: the NMI handler and the main loop both
    // read and write this.  The compiler must not optimize away
    // any read or write.
    volatile nmi_count: u8 = 0;

    // volatile flag: set by the NMI handler, polled by the main loop.
    volatile frame_ready: u8 = 0;

    // memory-mapped PPU register at a fixed address.
    // The variable name PPU_STATUS resolves to address 0x2002.
    #[addr(0x2002)]
    PPU_STATUS: u8;

    // memory-mapped PPU data port.
    #[addr(0x2007)]
    PPU_IO: u8;
}

// The interrupt handler increments the volatile counter.
#[interrupt(nmi)]
fn nmi_handler() {
    inc nmi_count
    lda 1
    sta frame_ready
}

// The main loop waits for the volatile flag.
noreturn fn main() {
    loop {
        // wait for the NMI handler to set frame_ready
        do {
            lda frame_ready
        } while (zero)
        lda 0
        sta frame_ready

        // read the PPU status register by its variable name.
        // The compiler knows PPU_STATUS is at 0x2002 because
        // of the #[addr(...)] attribute.
        lda PPU_STATUS

        // do frame work here
    }
}
```

### Array variables

An array variable declares a count in square brackets.

```
msgbuf: [u8; 64];
tile_data: [u8; 128] = [0; 128];
```

A program may omit the size if the declaration includes an initializer list.
The compiler infers the size from the initializer.

```
palette: [u8] = [0x0F, 0x30, 0x21, 0x22];
```

### Struct variable initialization

A struct variable initializer uses braces and lists the field values in order.

```
struct TIME { ticks: u8, seconds: u8, minutes: u8, hours: u8 }

now: TIME = { 0, 10, 1, 3 };
```

### In-function variable declarations

A function body may contain variable declarations. A declaration inside a
function body allocates space in the current RAM block at the current offset.
The `#[addr(value)]` attribute binds the variable to a fixed address.

```
fn loader() {
    #[addr(0x80)]
    exe_size: u16;

    #[addr(0x84)]
    exe_segment: u8;
}
```

### function declarations

A `fn` declaration defines a subroutine. The function body contains assembly
instructions, control-flow constructs, and calls to other functions. A function
takes no parameters.

```
fn foo() {
    lda 0x0200
    inx
    ora 0x0200
}
```

The `noreturn` keyword goes before the `fn` keyword, the same way `inline`
does. The compiler omits the return-from-subroutine instruction at the end of
the body.

```
noreturn fn main() {
    loop {
        // main loop
    }
}
```

The `return` keyword exits a function early. The compiler emits the
return-from-subroutine instruction.

```
fn foo() {
    lda 0x0200
    if (zero) {
        return
    }
    inx
}
```

### inline macro declarations

An `inline fn` declaration defines a macro. A macro takes a parameter list.
The compiler expands the macro body at each call site and substitutes the
parameter names with the call arguments.

```
inline fn assign(dest, value) {
    lda value
    sta dest
}
```

A program calls a macro with the same syntax as a function call.

```
fn foo() {
    assign(PPU::CNT0, 0)
}
```

The compiler expands the call to:

```
fn foo() {
    lda 0
    sta PPU::CNT0
}
```

### interrupt handler declarations

The `#[interrupt(name)]` attribute marks a function as an interrupt handler.
The name depends on the CPU family. The bank defines the valid names.

```
#[interrupt(reset)]
noreturn fn main() {
    loop {
        // main loop
    }
}

#[interrupt(nmi)]
fn nmi_handler() {
}

#[interrupt(irq)]
fn irq_handler() {
}
```

### struct declarations

A `struct` declaration defines a struct type as described in the Type system
section.

### type alias declarations

A `type` declaration defines a type alias as described in the Type system
section.

### enum declarations

An `enum` declaration defines a group of constants as described in the Type
system section.

### mod and use declarations

A `mod` declaration declares a module. A module groups declarations. A module
may reference another file or may contain a body block.

```
mod graphics;
mod audio {
    fn play_sound() { }
}
```

A `use` declaration imports a path into the current scope.

```
use nes::ppu;
use cpu::{a, x, y};
```

## Conditional compilation

The `#[cfg(predicate)]` attribute includes or excludes an item based on the
target and the feature flags. The compiler evaluates the predicate before it
parses the item body.

```
#[cfg(target = "mos6502-nintendo-nes-ntsc")]
const VIDEO_STANDARD: u8 = 0;

#[cfg(target = "mos6502-nintendo-nes-pal")]
const VIDEO_STANDARD: u8 = 1;
```

The cfg predicate supports the following keys.

| Key | Value | Example |
|-----|-------|---------|
| `target` | Full triplet string | `mos6502-nintendo-nes-ntsc` |
| `cpu` | CPU family name | `mos6502` |
| `manufacturer` | Manufacturer name | `nintendo` |
| `machine` | Machine name | `nes` |
| `variant` | Variant name | `ntsc` |
| `feature` | Feature flag name | `debug` |

The cfg predicate supports the boolean combinators `all`, `any`, and `not`.

```
#[cfg(all(cpu = "mos6502", not(variant = "pal")))]
const REFRESH_HZ: u8 = 60;

#[cfg(any(target = "mos6502-nintendo-nes-ntsc", target = "mos6502-nintendo-nes-pal"))]
const IS_NES: bool = true;
```

A program passes feature flags on the `opc` command line or in the
`Cart.toml` `[features]` section. The compiler defines the `target`, `cpu`,
`manufacturer`, `machine`, and `variant` keys from the target triplet.

## Memory model

### ROM blocks

The `#[rom(org = value, bank = value, maxsize = value)]` attribute begins a
ROM block. The block wraps the declarations that follow. The block ends at the
closing brace.

```
#[rom(org = 0xC000, bank = 0)] {
    #[interrupt(reset)]
    noreturn fn main() { }

    fn foo() { }
}
```

The `org` field sets the base address. The `bank` field selects the ROM bank.
The `maxsize` field sets the maximum byte count. The compiler emits an error
if the block content exceeds the maxsize.

### RAM blocks

The `#[ram(org = value, maxsize = value)]` attribute begins a RAM block. The
block wraps variable declarations.

```
#[ram(org = 0x0000, maxsize = 0x100)] {
    counter: u8;
    paddr: pointer;
    msgbuf: [u8; 64];
}
```

### CHR data blocks (NES)

The `#[chr(bank = value)]` attribute begins a CHR data block. The block
contains binary data from `locate_bytes!` or from byte initializers.

```
#[chr(bank = 0)] {
    locate_bytes!("font.chr")
}
```

### Alignment

The `#[align(value)]` attribute aligns the next item to a power-of-two
boundary.

```
#[align(0x100)]
#[rom(org = 0, bank = 0)] {
    locate_bytes!("loader.bin")
}
```

### Padding

The `#[setpad(value)]` attribute sets the byte value that fills unused space in
memory blocks. The value may be an integer or a one-byte string.

```
#[setpad(0xFF)]
mod game;
```

### NES iNES header

The `#[ines(...)]` attribute sets the iNES header fields. The attribute applies
to the root module.

```
#[ines(
    mapper = 0,
    mirroring = "vertical",
    battery = false,
    trainer = false,
    fourscreen = false,
)]
mod game;
```

### Atari Lynx .lnx header

The `#[lnx(...)]` attribute sets the .lnx header fields.

```
#[lnx(
    version = 1,
    name = "Demo Game",
    manufacturer = "Demo Studio",
    rotation = "none",
    banks = 1,
    block_count = 256,
    block_size = 2048,
)]
mod game;
```

The `#[loader]` attribute marks a function as the encrypted first-stage loader.

```
#[loader]
fn micro_loader() {
    // loader body
}
```

## Control flow

### if / else

The `if` statement tests a CPU condition and runs a body if the test is true.
The optional `else` block runs if the test is false.

```
if (set) {
    ora 0x0200
} else {
    and 0x0200
}
```

A single-statement form does not require braces.

```
if (set)
    ora 0x0200
else {
    and 0x0200
    eor 0x0200
}
```

The condition expression uses target-specific test keywords. The bank defines
the valid keywords. See the Per-target tables section.

The optional `near` or `far` keyword before the condition sets the branch
distance. `near` forces a short branch. `far` forces a long branch. If the
program omits both, the code generator selects the encoding.

```
if (near set) { }
if (far carry) { }
```

### while

The `while` statement loops while the condition is true. The condition is
tested before each iteration.

```
while (not zero) {
    adc 0x0200
    dex
}
```

### do / while

The `do` statement runs the body once, then loops while the condition is true.

```
do {
    lda PPU::STATUS
} while (is plus)
```

### loop

The `loop` statement loops without a condition. A `return` or a jump exits
the loop.

```
loop {
    wait_for(6)
    pal_animate()
}
```

### switch / case / default

The `switch` statement compares a register with several immediate values and
runs the matching case block. The `default` block runs if no case matches.
Cases do **not** fall through. A case block ends at the next `case` or
`default` or at the switch closing brace.

```
switch (cpu::x) {
    case 12 {
        lda 23
    }
    case 34 {
        lda 12
    }
    default {
        lda 0
    }
}
```

The switch expression uses the `cpu::` module path to name a register. The
bank defines the register constants.

### Labels

A label marks a branch target inside a function body. A label begins with a
single quote and ends with a colon. A label appears on the same line as the
statement that follows it.

```
fn foo() {
    'loop: lda 0x0200
    bne 'loop
}
```

A program references the label in a jump or branch operand with the same
single-quote prefix.

## Assembly statements

### Opcode form

An assembly statement begins with a CPU-specific opcode mnemonic. The opcode
may take zero or more operands. The bank defines the opcode set and the
legal addressing modes for each opcode.

```
lda 0x0200
inx
sta PPU::CNT0
jmp 'loop
```

### Immediate operands

An immediate operand begins with the `#` prefix. The value after `#` may be a
number literal, a const name, a compile-time macro call, or a constant
expression.

```
lda #0
lda #0xFF
lda #lo!(NAME_TABLE_0_ADDRESS)
cmp #0xA
```

The `#` prefix tells the assembler to encode the operand as an immediate value.
Without `#`, the assembler encodes the operand as a memory address.

### Addressing modes

The assembler selects the shortest encoding automatically. If the operand
value fits in one byte and the label is in zero page, the assembler uses the
zero-page form. Otherwise, the assembler uses the absolute form. For branch
instructions, the assembler uses the relative form.

The assembler emits a warning when the linker resolves a zero-page versus
absolute ambiguity.

A program may force an addressing mode with an explicit prefix. The bank
defines the prefix names. If the linker finds that the explicit mode is not the
optimal mode, the assembler emits a warning.

| Prefix | Mode |
|--------|------|
| `zp` | Zero page |
| `abs` | Absolute |
| `rel` | Relative (branch) |
| `ind` | Indirect |
| `idx` | Indexed |
| `ind_idx` | Indirect indexed |

Example:

```
lda zp 0x20
lda abs 0x2000
jmp ind 0xFFFD
lda idx 0x0200, x
lda ind_idx (0x20), y
```

### Operands

An operand may be a number, a selector, a label reference, a register
reference, or a parenthesized sub-expression.

A selector names a constant, a field, or an offset.

```
lda PPU::CNT0
lda player::x
lda dest + 1
lda src + 0
```

The `::` operator accesses a module or enum constant. The `.` operator accesses
a struct field. The `+` and `-` operators add or subtract a constant offset
from a base address.

## File inclusion

### Source inclusion

A `mod name;` declaration tells the compiler to find the file `name.op` and
parse it as a module.

A `use path;` declaration imports a path into the current scope.

### Binary inclusion

The `locate_bytes!("file.bin")` macro reads a binary file and places its bytes
into the current data block at the current offset.

```
#[chr(bank = 0)] {
    locate_bytes!("font.chr")
}
```

### Source inclusion at parse time

The `locate_str!("file.op")` macro reads a text file and parses it as Op
source at the current location.

### Function placement

The `locate_fn!(path::name)` macro places a function from another module
into the current ROM block. The function body is compiled and located at the
current section offset. An `#[interrupt(...)]` attribute on the `locate_fn!`
call maps the function to an interrupt vector.

```
#[rom(org = 0xC000, bank = 0, maxsize = 0x4000)] {

    #[interrupt(reset)]
    locate_fn!(game::main);

    #[interrupt(nmi)]
    locate_fn!(game::nmi_handler);

    #[interrupt(irq)]
    locate_fn!(game::irq_handler);
}
```

The `locate_fn!` macro lets the game file act as a declarative layout script.
The game file places functions, data, and binary blobs into ROM, RAM, and CHR
regions without containing the function bodies themselves.

## Expressions and constant evaluation

### Operator precedence

The operator precedence from lowest to highest is:

1. `\|` (binary or)
2. `^` (binary exclusive or)
3. `&` (binary and)
4. `==` `!=` (equality)
5. `<` `>` `<=` `>=` (comparison)
6. `<<` `>>` (shift)
7. `+` `-` (add, subtract)
8. `*` `/` `%` (multiply, divide, modulo)
9. `~` `!` (unary inverse, unary not)
10. `-` `+` (unary negative, unary positive)

Parentheses override the precedence.

### Compile-time macros

The `lo!`, `hi!`, `nylo!`, `nyhi!`, and `sizeof!` macros evaluate at compile
time. A program may call them in `const` expressions and in immediate operands.

```
const LO_ADDR: u8 = lo!(0x0200);
const HI_ADDR: u8 = hi!(0x0200);
const FN_SIZE: u8 = sizeof!(main);
const FILE_SIZE: u16 = sizeof!("game.bin");
```

## Banks

Each target ships as a set of banks. A CPU bank provides the `cpu` module with
register constants, the opcode encoding table, the condition keywords, and the
addressing mode definitions. A platform bank depends on a CPU bank and adds
the memory-mapped IO addresses, the platform constants, and the standard
inline macros.

Example bank names: `mos6502-bank`, `nes-bank`, `z80-bank`, `lynx-bank`.

The `cart` tool installs banks in `~/.carts/`. The `Cart.toml` `[dependencies]`
section lists the banks that a project uses. The `opc` compiler loads the
banks at build time. A program may declare and use its own banks with `use`
declarations.

## Target triplets

A target triplet has the form `cpu-manufacturer-machine-variant`. The compiler
selects the CPU bank, the platform bank, and the memory map from the triplet.

The following table lists the normative triplets.

| Triplet | CPU | Platform |
|---------|-----|----------|
| `mos6502-apple-ii` | MOS 6502 | Apple II |
| `mos6502-apple-iic` | MOS 6502 | Apple IIc |
| `mos6502-apple-iie` | MOS 6502 | Apple IIe |
| `mos6502-apple-iie-enhanced` | MOS 6502 | Apple IIe Enhanced |
| `mos6502-atari-800-ntsc` | MOS 6502 | Atari 800 NTSC |
| `mos6502-atari-800-pal` | MOS 6502 | Atari 800 PAL |
| `mos6502-atari-2600` | MOS 6502 | Atari 2600 |
| `mos6502-atari-5200` | MOS 6502 | Atari 5200 |
| `mos6502-atari-7800` | MOS 6502 | Atari 7800 |
| `mos65sc02-atari-lynx` | MOS 65SC02 | Atari Lynx |
| `mos6502-commodore-64` | MOS 6502 | Commodore 64 |
| `mos6502-nec-pcengine` | MOS 6502 | NEC PC Engine |
| `mos6502-nintendo-nes-ntsc` | Ricoh 2A03 | NES NTSC |
| `mos6502-nintendo-nes-pal` | Ricoh 2A07 | NES PAL |
| `m68000-neogeo-aes` | Motorola 68000 | Neo Geo AES |
| `m68000-sega-genesis` | Motorola 68000 | Sega Genesis |
| `wdc65c816-apple-iigs` | WDC 65C816 | Apple IIgs |
| `wdc65c816-nintendo-snes` | WDC 65C816 | SNES |
| `z80-neogeo-aes` | Zilog Z80 | Neo Geo AES |
| `z80-nintendo-gameboy` | Sharp LR35902 | Game Boy |
| `z80-nintendo-gameboy-color` | Sharp LR35902 | Game Boy Color |
| `z80-sega-gamegear` | Zilog Z80 | Sega Game Gear |
| `z80-sega-genesis` | Zilog Z80 | Sega Genesis |
| `z80-sega-mastersystem` | Zilog Z80 | Sega Master System |
| `z80-sega-sg1000` | Zilog Z80 | Sega SG-1000 |
| `z80-sinclair-zx80` | Zilog Z80 | Sinclair ZX80 |
| `z80-sinclair-zx81` | Zilog Z80 | Sinclair ZX81 |
| `z80-sinclair-spectrum` | Zilog Z80 | Sinclair Spectrum |
| `z80-ti-85` | Zilog Z80 | Texas Instruments TI-85 |

## Per-target tables

This section defines the condition keywords, the registers, the opcode tables,
and the addressing modes for each CPU family.

### MOS 6502

#### Status flags

The 6502 status register has seven bits: N (negative), V (overflow), B (break),
D (decimal), I (interrupt disable), Z (zero), C (carry).

#### Condition keywords

The bank defines the following condition keywords for `if`, `while`, and
`do-while` tests.

| Keyword | Flag tested | Branch opcode |
|---------|-------------|---------------|
| `plus` | N = 0 | BPL |
| `positive` | N = 0 | BPL |
| `minus` | N = 1 | BMI |
| `negative` | N = 1 | BMI |
| `greater` | N = 0 | BPL |
| `less` | N = 1 | BMI |
| `overflow` | V = 1 | BVS |
| `carry` | C = 1 | BCS |
| `nonzero` | Z = 0 | BNE |
| `set` | Z = 0 | BNE |
| `true` | Z = 0 | BNE |
| `zero` | Z = 1 | BEQ |
| `unset` | Z = 1 | BEQ |
| `false` | Z = 1 | BEQ |
| `clear` | Z = 1 | BEQ |
| `equal` | Z = 1 | BEQ |

The modifier keywords `is`, `has`, `no`, and `not` may prefix a condition to
improve readability. The compiler treats them as no-ops.

#### Registers

The bank defines `cpu::a`, `cpu::x`, and `cpu::y` as register references for
use in `switch` statements and operand expressions.

#### Opcodes

The 6502 opcode table lists each mnemonic and the legal addressing modes.

| Mnemonic | Addressing modes |
|----------|------------------|
| ADC | immediate, zero-page, zero-page-x, absolute, absolute-x, absolute-y, indirect-y, indirect-x |
| AND | immediate, zero-page, zero-page-x, absolute, absolute-x, absolute-y, indirect-y, indirect-x |
| ASL | accumulator, zero-page, zero-page-x, absolute, absolute-x |
| BCC | relative |
| BCS | relative |
| BEQ | relative |
| BIT | zero-page, absolute |
| BMI | relative |
| BNE | relative |
| BPL | relative |
| BRK | implied |
| BVC | relative |
| BVS | relative |
| CLC | implied |
| CLD | implied |
| CLI | implied |
| CLV | implied |
| CMP | immediate, zero-page, zero-page-x, absolute, absolute-x, absolute-y, indirect-y, indirect-x |
| CPX | immediate, zero-page, absolute |
| CPY | immediate, zero-page, absolute |
| DEC | zero-page, zero-page-x, absolute, absolute-x |
| DEX | implied |
| DEY | implied |
| EOR | immediate, zero-page, zero-page-x, absolute, absolute-x, absolute-y, indirect-y, indirect-x |
| INC | zero-page, zero-page-x, absolute, absolute-x |
| INX | implied |
| INY | implied |
| JMP | absolute, indirect |
| JSR | absolute |
| LDA | immediate, zero-page, zero-page-x, absolute, absolute-x, absolute-y, indirect-y, indirect-x |
| LDX | immediate, zero-page, zero-page-y, absolute, absolute-y |
| LDY | immediate, zero-page, zero-page-x, absolute, absolute-x |
| LSR | accumulator, zero-page, zero-page-x, absolute, absolute-x |
| NOP | implied |
| ORA | immediate, zero-page, zero-page-x, absolute, absolute-x, absolute-y, indirect-y, indirect-x |
| PHA | implied |
| PHP | implied |
| PLA | implied |
| PLP | implied |
| ROL | accumulator, zero-page, zero-page-x, absolute, absolute-x |
| ROR | accumulator, zero-page, zero-page-x, absolute, absolute-x |
| RTI | implied |
| RTS | implied |
| SBC | immediate, zero-page, zero-page-x, absolute, absolute-x, absolute-y, indirect-y, indirect-x |
| SEC | implied |
| SED | implied |
| SEI | implied |
| STA | zero-page, zero-page-x, absolute, absolute-x, absolute-y, indirect-y, indirect-x |
| STX | zero-page, zero-page-y, absolute |
| STY | zero-page, zero-page-x, absolute |
| TAX | implied |
| TAY | implied |
| TSX | implied |
| TXA | implied |
| TXS | implied |
| TYA | implied |

#### Undocumented opcodes

The 6502 has undocumented opcodes that real hardware executes. The bank
defines them as legal mnemonics. A program may use them with `#[cfg(feature =
"undocumented")]`.

| Mnemonic | Operation |
|----------|-----------|
| ALR | AND operand with A, then logical shift right A |
| ANC | AND operand with A, then copy bit 7 to carry |
| ANE | AND operand with A and X and magic constant |
| ARR | AND operand with A, then rotate right A |
| DCP | DEC operand, then CMP operand with A |
| ISC | INC operand, then SBC operand from A |
| LAS | AND operand with A and stack pointer, load A, X, SP |
| LAX | LDA and LDX in one instruction |
| LXA | AND operand with A and magic constant, load A and X |
| RLA | ROL operand, then AND operand with A |
| RRA | ROR operand, then ADC operand to A |
| SAX | AND A with X, store result in operand |
| SHA | Store A and high byte of address plus 1 |
| SHX | Store X and high byte of address plus 1 |
| SHY | Store Y and high byte of address plus 1 |
| SLO | ASL operand, then ORA operand with A |
| SRE | LSR operand, then EOR operand with A |
| TAS | AND A with X, store in SP and high byte of address |
| USBC | SBC without borrow (same as SBC with carry clear) |
| NOP | No-operation variants with extra operand bytes |

#### Interrupts

The 6502 supports three interrupt vectors. The bank defines the names `reset`,
`nmi`, and `irq` for `#[interrupt(...)]`.

### MOS 65SC02

The MOS 65SC02 extends the 6502 with additional opcodes and addressing modes.
The 65SC02 removes the undocumented opcodes.

#### Additional opcodes

| Mnemonic | Operation |
|----------|-----------|
| ADC | Adds zero-page-indirect addressing mode |
| AND | Adds zero-page-indirect addressing mode |
| BIT | Adds immediate, zero-page-x, absolute-x addressing modes |
| CMP | Adds zero-page-indirect addressing mode |
| EOR | Adds zero-page-indirect addressing mode |
| ORA | Adds zero-page-indirect addressing mode |
| SBC | Adds zero-page-indirect addressing mode |
| BRA | Branch always (relative) |
| PHX | Push X to stack (implied) |
| PHY | Push Y to stack (implied) |
| PLX | Pull X from stack (implied) |
| PLY | Pull Y from stack (implied) |
| STZ | Store zero (zero-page, zero-page-x, absolute, absolute-x) |
| TSB | Test and set bits (zero-page, absolute) |
| TRB | Test and reset bits (zero-page, absolute) |
| INA | Increment A (implied) |
| DEA | Decrement A (implied) |

#### Interrupts

The Atari Lynx uses the 65SC02. The Lynx does not directly address cartridge
ROM. All code runs from RAM. The Lynx supports the plain `interrupt` keyword
form. The bank defines `irq` for `#[interrupt(...)]`. The programmer sets the
interrupt vector registers at run time.

### Ricoh 2A03 / 2A07

The Ricoh 2A03 (NTSC) and 2A07 (PAL) are NMOS 6502 cores without binary-coded
decimal mode. The opcode set and the addressing modes match the MOS 6502. The
undocumented opcodes match the MOS 6502.

The 2A03 includes on-die audio and DMA hardware. The bank defines the
memory-mapped IO addresses for the audio and DMA registers.

The 2A07 differs from the 2A03 in the clock divider. The bank defines the
`REFRESH_HZ` constant as 60 for the 2A03 and 50 for the 2A07.

### WDC 65C816

The WDC 65C816 is a 16-bit extension of the 65C02. The CPU has an 8-bit
accumulator mode and a 16-bit accumulator mode. The CPU has an 8-bit index
mode and a 16-bit index mode.

#### Additional registers

The bank defines `cpu::a`, `cpu::x`, `cpu::y`, `cpu::dbr` (data bank
register), `cpu::pbr` (program bank register), and `cpu::dp` (direct page
register).

#### Additional opcodes

| Mnemonic | Operation |
|----------|-----------|
| REP | Reset processor status bits (immediate) |
| SEP | Set processor status bits (immediate) |
| XBA | Exchange B and A (implied) |
| XCE | Exchange carry and emulation bits (implied) |
| TCD | Transfer A to direct page register (implied) |
| TDC | Transfer direct page register to A (implied) |
| TCS | Transfer A to stack pointer (implied) |
| TSC | Transfer stack pointer to A (implied) |
| TXY | Transfer X to Y (implied) |
| TYX | Transfer Y to X (implied) |
| MVN | Block move negative (implied, source and dest banks) |
| MVP | Block move positive (implied, source and dest banks) |
| PEA | Push effective address (absolute) |
| PEI | Push effective indirect address (indirect) |
| PER | Push effective relative address (relative long) |
| JML | Jump long (absolute long) |
| JSL | Jump to subroutine long (absolute long) |
| RTL | Return from subroutine long (implied) |
| COP | Co-processor (implied) |
| WAI | Wait for interrupt (implied) |
| STP | Stop processor (implied) |

#### Addressing modes

The 65C816 adds absolute long, absolute long indexed X, direct page, direct
page indirect, direct page indirect long, stack relative, stack relative
indirect indexed Y, and block move addressing modes to the 65SC02 set.

### Motorola 68000

The Motorola 68000 is a 32-bit CISC CPU. The CPU has eight 32-bit data
registers and eight 32-bit address registers.

#### Registers

The bank defines `cpu::d0` through `cpu::d7` as data registers and `cpu::a0`
through `cpu::a7` as address registers. The bank defines `cpu::usp` (user
stack pointer), `cpu::ssp` (supervisor stack pointer), and `cpu::pc` (program
counter).

#### Condition keywords

| Keyword | Condition code |
|---------|----------------|
| `true` | T (always) |
| `false` | F (never) |
| `high` | HI (high) |
| `low_or_same` | LS |
| `carry_clear` | CC (carry clear) |
| `carry_set` | CS (carry set) |
| `not_equal` | NE |
| `equal` | EQ |
| `overflow_clear` | VC |
| `overflow_set` | VS |
| `plus` | PL (plus) |
| `minus` | MI (minus) |
| `greater_or_equal` | GE |
| `less_than` | LT |
| `greater_than` | GT |
| `less_or_equal` | LE |

#### Opcodes

| Mnemonic | Operation |
|----------|-----------|
| MOVE | Move data |
| MOVEQ | Move quick |
| MOVEM | Move multiple registers |
| LEA | Load effective address |
| PEA | Push effective address |
| CLR | Clear operand |
| NOT | Logical not |
| AND | Logical and |
| OR | Logical or |
| EOR | Logical exclusive or |
| ADD | Add |
| ADDA | Add address |
| ADDI | Add immediate |
| ADDQ | Add quick |
| SUB | Subtract |
| SUBA | Subtract address |
| SUBI | Subtract immediate |
| SUBQ | Subtract quick |
| MULU | Multiply unsigned |
| MULS | Multiply signed |
| DIVU | Divide unsigned |
| DIVS | Divide signed |
| NEG | Negate |
| NEGX | Negate with extend |
| ABS | Absolute value |
| ASL | Arithmetic shift left |
| ASR | Arithmetic shift right |
| LSL | Logical shift left |
| LSR | Logical shift right |
| ROL | Rotate left |
| ROR | Rotate right |
| ROXL | Rotate left with extend |
| ROXR | Rotate right with extend |
| CMP | Compare |
| CMPA | Compare address |
| CMPI | Compare immediate |
| TST | Test operand |
| BTST | Test bit |
| BSET | Test and set bit |
| BCLR | Test and clear bit |
| BCHG | Test and change bit |
| JMP | Jump |
| JSR | Jump to subroutine |
| RTS | Return from subroutine |
| RTR | Return and restore |
| RTE | Return from exception |
| Bcc | Branch conditional |
| BRA | Branch always |
| BSR | Branch to subroutine |
| DBcc | Decrement and branch conditional |
| CHK | Check register against bounds |
| TRAP | Trap |
| TRAPV | Trap on overflow |
| SWAP | Swap word halves of register |
| EXG | Exchange registers |
| EXT | Sign extend |
| LINK | Link stack |
| UNLK | Unlink stack |
| RESET | Reset external devices |
| NOP | No operation |
| STOP | Stop processor |
| ILLEGAL | Illegal instruction |

#### Interrupts

The 68000 uses an interrupt priority level system. The bank defines
`#[interrupt(level)]` where level is 1 to 7. The bank also defines
`#[interrupt(trap)]` for trap vectors.

### Zilog Z80

The Zilog Z80 is an 8-bit CPU. The CPU extends the Intel 8080 instruction set.

#### Registers

The bank defines `cpu::a`, `cpu::b`, `cpu::c`, `cpu::d`, `cpu::e`, `cpu::h`,
`cpu::l` as 8-bit registers. The bank defines `cpu::af`, `cpu::bc`, `cpu::de`,
`cpu::hl` as 16-bit register pairs. The bank defines `cpu::ix`, `cpu::iy` as
16-bit index registers. The bank defines `cpu::sp` (stack pointer) and
`cpu::pc` (program counter). The bank defines the alternate register set
`cpu::af2`, `cpu::bc2`, `cpu::de2`, `cpu::hl2`.

#### Condition keywords

| Keyword | Flag | Condition |
|---------|------|-----------|
| `not_zero` | Z = 0 | NZ |
| `zero` | Z = 1 | Z |
| `no_carry` | C = 0 | NC |
| `carry` | C = 1 | C |
| `parity_even` | P/V = 1 | PO |
| `parity_odd` | P/V = 0 | PE |
| `sign_positive` | S = 0 | P |
| `sign_negative` | S = 1 | M |

#### Opcodes

| Mnemonic | Operation |
|----------|-----------|
| LD | Load |
| PUSH | Push register pair |
| POP | Pop register pair |
| EX | Exchange |
| EXX | Exchange alternate register set |
| LDI | Load and increment |
| LDIR | Load, increment, repeat |
| LDD | Load and decrement |
| LDDR | Load, decrement, repeat |
| CPI | Compare and increment |
| CPIR | Compare, increment, repeat |
| CPD | Compare and decrement |
| CPDR | Compare, decrement, repeat |
| ADD | Add |
| ADC | Add with carry |
| SUB | Subtract |
| SBC | Subtract with carry |
| AND | Logical and |
| OR | Logical or |
| XOR | Logical exclusive or |
| CP | Compare |
| INC | Increment |
| DEC | Decrement |
| DAA | Decimal adjust accumulator |
| CPL | Complement accumulator |
| NEG | Negate accumulator |
| CCF | Complement carry flag |
| SCF | Set carry flag |
| NOP | No operation |
| HALT | Halt |
| DI | Disable interrupts |
| EI | Enable interrupts |
| IM | Set interrupt mode |
| RLC | Rotate left circular |
| RL | Rotate left through carry |
| RRC | Rotate right circular |
| RR | Rotate right through carry |
| SLA | Shift left arithmetic |
| SRA | Shift right arithmetic |
| SLL | Shift left logical |
| SRL | Shift right logical |
| RLD | Rotate left digit |
| RRD | Rotate right digit |
| RLCA | Rotate accumulator left circular |
| RLA | Rotate accumulator left |
| RRCA | Rotate accumulator right circular |
| RRA | Rotate accumulator right |
| JP | Jump |
| JR | Jump relative |
| DJNZ | Decrement B and jump if not zero |
| CALL | Call subroutine |
| RET | Return |
| RETI | Return from interrupt |
| RETN | Return from non-maskable interrupt |
| RST | Restart |
| IN | Input |
| OUT | Output |
| INI | Input and increment |
| INIR | Input, increment, repeat |
| IND | Input and decrement |
| INDR | Input, decrement, repeat |
| OUTI | Output and increment |
| OTIR | Output, increment, repeat |
| OUTD | Output and decrement |
| OTDR | Output, decrement, repeat |
| BIT | Test bit |
| SET | Set bit |
| RES | Reset bit |

#### Undocumented opcodes

| Mnemonic | Operation |
|----------|-----------|
| SLL | Shift left logical (bit 0 set to 1) |
| IN (C) | Input to all registers when B is not used as operand |

### Sharp LR35902

The Sharp LR35902 is a Z80 variant used in the Nintendo Game Boy and Game Boy
Color. The CPU removes the alternate register set, the I and R registers, and
several Z80 opcodes. The CPU adds the `STOP` and `LDI A, (HL)` opcodes.

#### Registers

The bank defines `cpu::a`, `cpu::b`, `cpu::c`, `cpu::d`, `cpu::e`, `cpu::h`,
`cpu::l` as 8-bit registers. The bank defines `cpu::af`, `cpu::bc`, `cpu::de`,
`cpu::hl` as 16-bit register pairs. The bank defines `cpu::sp` and `cpu::pc`.

#### Opcodes

The LR35902 opcode set is a subset of the Z80 set. The bank defines the legal
opcodes. Additional opcodes: `STOP`, `LDI A`, `LDD A`, `LDH A`, `LDH (n)`.

The LR35902 does **not** support the alternate register set, the `EXX`
instruction, the `RLD` and `RRD` instructions, and the interrupt mode
selection is simplified.

## Grammar

This section defines the normalized LR(1) grammar. The grammar uses Extended
Backus-Naur Form. Square brackets denote optional elements. Curly braces with
a star denote zero or more repetitions. A vertical bar separates alternatives.

### Lexical productions

```
program        ::= module_item*

module_item    ::= attribute* item

item           ::= const_decl
                 | var_decl
                 | fn_decl
                 | inline_fn_decl
                 | struct_decl
                 | type_decl
                 | enum_decl
                 | mod_decl
                 | use_decl
                 | block_attribute
                 | module_doc_comment

attribute      ::= '#' '[' attr_path attr_args? ']'
attr_path      ::= IDENTIFIER ('::' IDENTIFIER)*
attr_args      ::= '(' attr_arg (',' attr_arg)* ')'
attr_arg       ::= IDENTIFIER ('=' literal)?

const_decl     ::= 'const' IDENTIFIER ':' type '=' expr ';'

var_decl        ::= 'volatile'? IDENTIFIER ':' type
                    array_dim? addr_binding? init_value? ';'

addr_binding   ::= ':' expr
init_value     ::= '=' (expr | init_list | string_literal)

fn_decl        ::= 'noreturn'? 'fn' IDENTIFIER '(' ')' '{' fn_body '}'

inline_fn_decl ::= 'inline' 'fn' IDENTIFIER '(' param_list ')' '{' fn_body '}'

param_list     ::= IDENTIFIER (',' IDENTIFIER)*

struct_decl    ::= 'struct' IDENTIFIER '{' field_list '}'
field_list     ::= field (',' field)*
field          ::= 'volatile'? IDENTIFIER ':' type array_dim?

type_decl      ::= 'type' IDENTIFIER '=' type ';'

enum_decl      ::= 'enum' IDENTIFIER '{' enum_variant (',' enum_variant)* '}'
enum_variant   ::= IDENTIFIER '=' expr

mod_decl       ::= 'mod' IDENTIFIER (';' | '{' module_item* '}')

use_decl       ::= 'use' use_path (',' use_path)* ';'
use_path       ::= IDENTIFIER ('::' IDENTIFIER)*

block_attribute ::= '#[' block_attr_name '(' attr_args ')' ']' '{' module_item* '}'

type           ::= IDENTIFIER
                 | '[' type ';' expr ']'

array_dim      ::= '[' expr? ']'

literal        ::= NUMBER | STRING | 'true' | 'false'
```

### Function body productions

```
fn_body        ::= fn_stmt*

fn_stmt        ::= label
                 | assembly_stmt
                 | if_stmt
                 | while_stmt
                 | do_while_stmt
                 | loop_stmt
                 | switch_stmt
                 | fn_call
                 | 'return'
                 | var_decl

label          ::= '\'' IDENTIFIER ':' assembly_stmt
                 | '\'' IDENTIFIER ':' control_stmt

assembly_stmt  ::= OPCODE operand*
                 | OPCODE

operand        ::= immediate
                 | memory_operand
                 | register_ref
                 | label_ref
                 | selector

immediate      ::= '#' expr

memory_operand ::= mode_prefix? expr
                 | mode_prefix? '(' expr ')' index_reg?
                 | mode_prefix? expr ',' index_reg
                 | mode_prefix? '(' expr ',' index_reg ')'
                 | mode_prefix? '(' expr ')' ',' index_reg

mode_prefix    ::= 'zp' | 'abs' | 'rel' | 'ind' | 'idx' | 'ind_l' | 'ind_idx'

index_reg      ::= 'cpu::a' | 'cpu::x' | 'cpu::y' | 'cpu::d0' ... (target-defined)

register_ref   ::= 'cpu::' IDENTIFIER

label_ref      ::= '\'' IDENTIFIER

selector       ::= IDENTIFIER ('::' IDENTIFIER)* ('.' IDENTIFIER)*
                    (('+' | '-') expr)*
```

### Control-flow productions

```
if_stmt        ::= 'if' '(' branch_hint? condition ')' block else_block?
                 | 'if' '(' branch_hint? condition ')' fn_stmt else_block?

branch_hint    ::= 'near' | 'far'

else_block     ::= 'else' block
                 | 'else' fn_stmt

while_stmt     ::= 'while' '(' branch_hint? condition ')' block
                 | 'while' '(' branch_hint? condition ')' fn_stmt

do_while_stmt  ::= 'do' block 'while' '(' branch_hint? condition ')'
                 | 'do' fn_stmt 'while' '(' branch_hint? condition ')'

loop_stmt      ::= 'loop' block
                 | 'loop' fn_stmt

switch_stmt    ::= 'switch' '(' register_ref ')' '{' switch_case* '}'

switch_case    ::= 'case' expr block
                 | 'case' expr fn_stmt
                 | 'default' block
                 | 'default' fn_stmt

condition      ::= modifier* CONDITION_KEYWORD
modifier       ::= 'is' | 'has' | 'no' | 'not'

block          ::= '{' fn_body '}'

fn_call        ::= IDENTIFIER '(' arg_list? ')'
arg_list       ::= expr (',' expr)*
```

### Expression productions

```
expr           ::= or_expr
or_expr        ::= xor_expr ('|' xor_expr)*
xor_expr       ::= and_expr ('^' and_expr)*
and_expr       ::= eq_expr ('&' eq_expr)*
eq_expr        ::= cmp_expr (('==' | '!=') cmp_expr)*
cmp_expr      ::= shift_expr (('<' | '>' | '<=' | '>=') shift_expr)*
shift_expr     ::= add_expr (('<<' | '>>') add_expr)*
add_expr       ::= mul_expr (('+' | '-') mul_expr)*
mul_expr       ::= unary_expr (('*' | '/' | '%') unary_expr)*
unary_expr     ::= ('~' | '!' | '-' | '+') unary_expr
                 | primary

primary        ::= NUMBER
                 | STRING
                 | 'true'
                 | 'false'
                 | IDENTIFIER
                 | selector
                 | '(' expr ')'
                 | compile_time_macro '!' '(' expr ')'

compile_time_macro ::= 'lo' | 'hi' | 'nylo' | 'nyhi' | 'sizeof'
```

## Conformance

A conforming implementation of the Op language must:

1. Accept all source files that the grammar in this document defines.
2. Reject source files that the grammar does not accept with a structured
   diagnostic.
3. Implement the type system as this document defines.
4. Implement the conditional compilation system as this document defines.
5. Implement the const expression evaluator as this document defines.
6. Load the banks that match the target triplet from `~/.carts/`.
7. Generate correct machine code for at least one target triplet in the
   normative table.

A conforming program must:

1. Use only the grammar that this document defines.
2. Use only the types that the target supports.
3. Use only the opcodes, registers, and condition keywords that the target
   bank defines.
4. Use only the target triplets that this document lists.

## Future work

The following items are not part of this specification. A future revision may
define them.

1. Block rearrangement optimization.
2. Dead-code elimination across function boundaries.
3. Jump threading.
4. Constant propagation across function calls.
5. A foreign-function interface for linking with C or assembly object files.
6. A package manager registry protocol for banks beyond git URLs.