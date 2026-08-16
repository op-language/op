# Op Library Binary (.opb) File Format

Version 1.0

This document defines the `.opb` file format. The `cart build` command
invokes the `opc` compiler to build a lib project. The `opc` compiler
writes the `.opb` file to `target/<triplet>/<libname>.opb`. The linker
reads the `.opb` file when a ROM project depends on the lib.

This document uses the keywords **must**, **shall**, and **may** as RFC
2119 defines.

## Scope

This document defines the binary layout of the `.opb` file and the JSON
layout of the `.opx` file. For the `.opb` file, the document defines the
header, the section table, the symbol table, the relocation table, and the
data blocks. For the `.opx` file, the document defines the envelope fields,
the token fields, and the token type reference table.

This document does not define the `.opa` or `.opl` intermediate file
formats. The document `technical-design.md` in the `op` repository defines
those formats. This document does not define the Op language grammar. The
document `language-specification.md` defines the grammar.

## Purpose

A lib project contains reusable code and data. When a ROM project depends
on a lib, the linker must merge the lib sections with the ROM sections.
The linker must resolve symbol references from the ROM code to the lib
symbols. The `.opb` file carries the compiled lib data in a binary form
that the linker reads quickly.

The `.opb` format differs from the `.opl` post-compile format in two
ways. First, the `.opb` format uses a binary layout, not JSON. Second,
the `.opb` file has a header that identifies the lib, the target, and
the format version.

## File structure

A `.opb` file has five parts. The parts appear in this order:

1. The file header.
2. The section table.
3. The symbol table.
4. The relocation table.
5. The data blocks.

All multi-byte integer fields use little-endian byte order.

## File header

The file header is 64 bytes. The header identifies the file and gives
the counts and offsets for the tables.

| Offset | Size | Field | Description |
|--------|------|-------|-------------|
| 0 | 4 | magic | The ASCII bytes `4F 50 42 21` (`OPB!`). |
| 4 | 4 | format_version | The format version. Current value: `2`. |
| 8 | 4 | target_len | The length in bytes of the target triplet string. |
| 12 | 4 | lib_name_len | The length in bytes of the lib name string. |
| 16 | 4 | section_count | The number of entries in the section table. |
| 20 | 4 | symbol_count | The number of entries in the symbol table. |
| 24 | 4 | reloc_count | The number of entries in the relocation table. |
| 28 | 4 | section_table_offset | The byte offset of the section table from the start of the file. |
| 32 | 4 | symbol_table_offset | The byte offset of the symbol table from the start of the file. |
| 36 | 4 | reloc_table_offset | The byte offset of the relocation table from the start of the file. |
| 40 | 4 | data_offset | The byte offset of the first data block from the start of the file. |
| 44 | 4 | reserved | Reserved for future use. Set to `0`. |
| 48 | 16 | padding | Zero bytes. Pad the header to 64 bytes. |

The target triplet string and the lib name string follow the header.
The target string starts at byte 64. The lib name string starts at byte
`64 + target_len`. The strings are not null-terminated.

## Section table

The section table starts at the offset that the header
`section_table_offset` field gives. Each entry is 32 bytes.

| Offset | Size | Field | Description |
|--------|------|-------|-------------|
| 0 | 4 | name_len | The length in bytes of the section name string. |
| 4 | 4 | kind | The section kind. See the table below. |
| 8 | 4 | org | The origin address of the section. |
| 12 | 4 | bank | The bank number of the section. |
| 16 | 4 | maxsize | The maximum size in bytes of the section. |
| 20 | 4 | data_size | The size in bytes of the section data. |
| 24 | 4 | data_offset | The byte offset of the data block from the start of the file. |
| 28 | 4 | reserved | Reserved for future use. Set to `0`. |

The section name string follows the entry. The string starts at
`section_table_offset + (index * 32) + 32`. The string is not
null-terminated. The `name_len` field gives the string length.

### Section kind values

| Value | Kind | Description |
|-------|------|-------------|
| `0` | `rom` | Read-only memory section. |
| `1` | `ram` | Random-access memory section. |
| `2` | `chr` | Character ROM section (NES). |

## Symbol table

The symbol table starts at the offset that the header
`symbol_table_offset` field gives. Each entry is 24 bytes.

| Offset | Size | Field | Description |
|--------|------|-------|-------------|
| 0 | 4 | name_len | The length in bytes of the symbol name string. |
| 4 | 4 | section_index | The index of the section that holds the symbol. The index is a zero-based offset into the section table. |
| 8 | 4 | offset | The offset of the symbol from the section origin. |
| 12 | 4 | size | The size of the symbol in bytes. |
| 16 | 4 | kind | The symbol kind. See the table below. |
| 20 | 4 | flags | Bit flags. Bit 0: `pub` (1 = the symbol is public and visible across lib boundaries; 0 = private). Bits 1-31: reserved, set to `0`. |

The symbol name string follows the entry. The string starts at
`symbol_table_offset + (index * 24) + 24`. The string is not
null-terminated. The `name_len` field gives the string length.

### Symbol kind values

| Value | Kind | Description |
|-------|------|-------------|
| `0` | `function` | A function entry point. |
| `1` | `variable` | A data variable. |
| `2` | `label` | A code label. |

## Relocation table

The relocation table starts at the offset that the header
`reloc_table_offset` field gives. Each entry is 16 bytes.

| Offset | Size | Field | Description |
|--------|------|-------|-------------|
| 0 | 4 | section_index | The index of the section that holds the relocation. The index is a zero-based offset into the section table. |
| 4 | 4 | offset | The byte offset within the section data where the linker must patch the value. |
| 8 | 4 | kind | The relocation kind. See the table below. |
| 12 | 4 | symbol_index | The index of the symbol that the relocation references. The index is a zero-based offset into the symbol table. |

### Relocation kind values

| Value | Kind | Size | Description |
|-------|------|------|-------------|
| `0` | `abs8` | 1 byte | Absolute 8-bit address. |
| `1` | `abs16` | 2 bytes | Absolute 16-bit address. |
| `2` | `abs24` | 3 bytes | Absolute 24-bit address (65C816). |
| `3` | `abs32` | 4 bytes | Absolute 32-bit address (68000). |
| `4` | `branch8` | 1 byte | Relative branch offset, 8-bit signed. |
| `5` | `branch16` | 2 bytes | Relative branch offset, 16-bit signed. |
| `6` | `lo8` | 1 byte | Low byte of a 16-bit symbol address. |
| `7` | `hi8` | 1 byte | High byte of a 16-bit symbol address. |
| `8` | `bank` | 1 byte | Bank number of a symbol (Lynx). |

## Data blocks

The data blocks start at the offset that the header `data_offset` field
gives. Each section has one data block. The section table entry
`data_offset` field gives the start of the block. The section table
entry `data_size` field gives the length of the block.

The data block contains the raw bytes of the compiled code and data for
the section. The linker copies these bytes into the final ROM image at
the section origin address.

## Linker use

When the linker links a ROM project that depends on a lib, the linker
must do these steps:

1. Read the `.opb` file from `target/<triplet>/<libname>.opb`.
2. Read the file header. Check the magic bytes. Check the format
   version.
3. Read the target triplet string. Check that it matches the ROM target
   triplet. If the target does not match, report an error.
4. Read the section table. For each section, read the name, kind, org,
   bank, maxsize, and data offset.
5. Read the symbol table. For each symbol, read the name, section index,
   offset, size, kind, and flags.
6. For each symbol, read the `flags` field. A symbol with the `pub` bit
   clear is private to the lib. The linker must not resolve a private
   symbol against references from other libs or ROMs. The linker skips
   private symbols when it resolves cross-lib references.
7. Read the relocation table. For each relocation, read the section
   index, offset, kind, and symbol index.
8. Read each data block.
9. Merge the lib sections with the ROM sections. The linker concatenates
   the data of sections that have the same name and bank. The linker
   adjusts the symbol offsets.
10. Resolve the lib relocations. The linker computes the final address of
    each lib symbol from the section origin and the symbol offset.
11. Patch the lib relocations in the merged data.

## File layout diagram

```
+------------------+
| File header      |  64 bytes
+------------------+
| Target triplet   |  target_len bytes
+------------------+
| Lib name         |  lib_name_len bytes
+------------------+
| Section table    |  section_count * 32 bytes
+------------------+
| Symbol table     |  symbol_count * 24 bytes
+------------------+
| Relocation table |  reloc_count * 16 bytes
+------------------+
| Data blocks      |  sum of all section data_size
+------------------+
```

## Conformance

A conforming `opc` implementation must:

1. Write the `.opb` file when `cart build` builds a lib project.
2. Set the magic bytes to `4F 50 42 21`.
3. Set the format version to `2`.
4. Write all multi-byte integer fields in little-endian byte order.
5. Write the target triplet string and the lib name string after the
   header.
6. Write the section table, the symbol table, the relocation table, and
   the data blocks at the offsets that the header gives.
7. Set the `pub` bit in the `flags` field of each symbol table entry to 1
   if the symbol is public. Set the bit to 0 if the symbol is private.
8. Set all reserved fields to `0`.

A conforming linker implementation must:

1. Read the `.opb` file when a ROM project depends on a lib.
2. Check the magic bytes and the format version.
3. Check that the target triplet matches the ROM target triplet.
4. Read the section table, the symbol table, the relocation table, and
   the data blocks.
5. Read the `flags` field of each symbol table entry. Resolve only public
   symbols across lib boundaries.
6. Merge the lib sections with the ROM sections.
7. Resolve and patch the lib relocations.

## Op Token Stream (.opx) File Format

Version 1.0

### Purpose

The `.opx` file is the JSON output of the `opc --lex` stage. The file
holds the token stream that the parser stage reads. A developer can inspect
the `.opx` file to debug the lexer output.

### Format

The `.opx` file is a JSON document. The file uses UTF-8 encoding. The
`opc` binary writes the file with pretty-printed JSON (two-space
indentation).

### Envelope fields

The top-level JSON object has three fields.

| Field | Type | Description |
|-------|------|-------------|
| `version` | integer | The format version. Current value: `1`. |
| `file` | string | The source file path that the lexer read. |
| `tokens` | array | The token list. Each element is a token object. |

### Token fields

Each element of the `tokens` array is a JSON object with four fields.

| Field | Type | Description |
|-------|------|-------------|
| `type` | string | The token type name. See the token type reference table below. |
| `value` | string | The source text of the token. |
| `line` | integer | The 1-based line number where the token starts. |
| `col` | integer | The 1-based column number where the token starts. |

### Example

Source file `main.op`:

```
fn main() {
```

`.opx` output:

```json
{
  "version": 1,
  "file": "main.op",
  "tokens": [
    { "type": "Kw_fn", "value": "fn", "line": 1, "col": 1 },
    { "type": "IDENT", "value": "main", "line": 1, "col": 4 },
    { "type": "Op_lparen", "value": "(", "line": 1, "col": 8 },
    { "type": "Op_rparen", "value": ")", "line": 1, "col": 9 },
    { "type": "Op_lbrace", "value": "{", "line": 1, "col": 11 }
  ]
}
```

### Token type reference table

The lexer emits one token type for each token it reads. The table below
lists every token type and its meaning.

#### Keywords

| Token type | Source text | Meaning |
|------------|-------------|---------|
| `Kw_fn` | `fn` | Declare a function |
| `Kw_inline` | `inline` | Declare an inline macro function |
| `Kw_noreturn` | `noreturn` | Mark a function that does not return |
| `Kw_return` | `return` | Exit a function early |
| `Kw_volatile` | `volatile` | Mark a variable as volatile |
| `Kw_struct` | `struct` | Declare a struct type |
| `Kw_type` | `type` | Declare a type alias |
| `Kw_enum` | `enum` | Declare an enum with discriminants |
| `Kw_const` | `const` | Declare a compile-time constant |
| `Kw_mod` | `mod` | Declare a module |
| `Kw_use` | `use` | Import a module path |
| `Kw_pub` | `pub` | Mark a module item as public |
| `Kw_if` | `if` | Begin a conditional block |
| `Kw_else` | `else` | Begin the alternate branch of an if block |
| `Kw_while` | `while` | Begin a while loop |
| `Kw_do` | `do` | Begin a do-while loop |
| `Kw_loop` | `loop` | Begin an endless loop |
| `Kw_switch` | `switch` | Begin a switch block |
| `Kw_case` | `case` | Begin a case block inside a switch |
| `Kw_default` | `default` | Begin the default block inside a switch |
| `Kw_near` | `near` | Force a short branch |
| `Kw_far` | `far` | Force a long branch |
| `Kw_as` | `as` | Cast or alias in certain contexts |
| `Kw_lib` | `lib` | Path root: the current lib root |
| `Kw_self` | `self` | Path root: the current module |
| `Kw_super` | `super` | Path root: the parent module |
| `Kw_true` | `true` | Boolean true literal |
| `Kw_false` | `false` | Boolean false literal |

#### Primitive types

| Token type | Source text | Meaning |
|------------|-------------|---------|
| `Type_u8` | `u8` | Unsigned 8-bit integer |
| `Type_i8` | `i8` | Signed 8-bit integer |
| `Type_u16` | `u16` | Unsigned 16-bit integer |
| `Type_i16` | `i16` | Signed 16-bit integer |
| `Type_u32` | `u32` | Unsigned 32-bit integer |
| `Type_i32` | `i32` | Signed 32-bit integer |
| `Type_bool` | `bool` | Boolean type |
| `Type_pointer` | `pointer` | Target-defined pointer type |

#### Operators and punctuation

| Token type | Source text | Meaning |
|------------|-------------|---------|
| `Op_hash` | `#` | Immediate-operand prefix |
| `Op_colon_colon` | `::` | Module or enum path separator |
| `Op_dot` | `.` | Struct field access |
| `Op_plus` | `+` | Add, unary positive, or offset |
| `Op_minus` | `-` | Subtract, unary negative, or offset |
| `Op_star` | `*` | Multiply |
| `Op_slash` | `/` | Divide |
| `Op_percent` | `%` | Modulo or binary prefix |
| `Op_tilde` | `~` | Binary inverse |
| `Op_bang` | `!` | Logical not or macro call suffix |
| `Op_amp` | `&` | Binary and |
| `Op_caret` | `^` | Binary exclusive or |
| `Op_pipe` | `\|` | Binary or |
| `Op_shr` | `>>` | Shift right |
| `Op_shl` | `<<` | Shift left |
| `Op_gt` | `>` | Greater than |
| `Op_lt` | `<` | Less than |
| `Op_ge` | `>=` | Greater than or equal |
| `Op_le` | `<=` | Less than or equal |
| `Op_eq` | `==` | Equal |
| `Op_ne` | `!=` | Not equal |
| `Op_assign` | `=` | Assignment in declarations |
| `Op_lparen` | `(` | Left parenthesis |
| `Op_rparen` | `)` | Right parenthesis |
| `Op_lbrace` | `{` | Left brace |
| `Op_rbrace` | `}` | Right brace |
| `Op_lbracket` | `[` | Left bracket |
| `Op_rbracket` | `]` | Right bracket |
| `Op_colon` | `:` | Type annotation or address binding |
| `Op_comma` | `,` | Separator in lists |
| `Op_semicolon` | `;` | Statement terminator |

#### Literals and identifiers

| Token type | Source text | Meaning |
|------------|-------------|---------|
| `NUMBER` | `123`, `%1010`, `0xFF` | A number literal (decimal, binary, or hex) |
| `STRING` | `"..."` | A string literal |
| `IDENT` | `[a-zA-Z_][a-zA-Z0-9_]*` | An identifier that is not a keyword, type, opcode, condition, modifier, mode prefix, or macro |
| `OPCODE` | `lda`, `sta`, `ld`, etc. | A CPU opcode mnemonic. The lexer matches opcodes case-insensitively. |
| `LABEL_DEF` | `'name:` | A label definition. The value includes the leading single quote and the trailing colon. |
| `LABEL_REF` | `'name` | A label reference. The value includes the leading single quote. |

#### Compile-time macros

| Token type | Source text | Meaning |
|------------|-------------|---------|
| `Macro_lo` | `lo!` | Low byte of a word, or low word of a dword |
| `Macro_hi` | `hi!` | High byte of a word, or high word of a dword |
| `Macro_nylo` | `nylo!` | Low nibble of a byte |
| `Macro_nyhi` | `nyhi!` | High nibble of a byte |
| `Macro_sizeof` | `sizeof!` | Byte size of a variable, type, function, or file |

#### Include macros

| Token type | Source text | Meaning |
|------------|-------------|---------|
| `Include_locate_bytes` | `locate_bytes!` | Read a binary file and place its bytes into the current data block |
| `Include_locate_str` | `locate_str!` | Read a text file and parse it as Op source at the current location |
| `Include_locate_fn` | `locate_fn!` | Place a function from another module into the current ROM block |

#### Condition keywords

| Token type | Source text | Meaning |
|------------|-------------|---------|
| `Cond_plus` | `plus` | 6502: N flag is 0 |
| `Cond_positive` | `positive` | 6502: N flag is 0 |
| `Cond_minus` | `minus` | 6502: N flag is 1 |
| `Cond_negative` | `negative` | 6502: N flag is 1 |
| `Cond_greater` | `greater` | 6502: N flag is 0 |
| `Cond_less` | `less` | 6502: N flag is 1 |
| `Cond_overflow` | `overflow` | 6502: V flag is 1 |
| `Cond_carry` | `carry` | 6502: C flag is 1 |
| `Cond_nonzero` | `nonzero` | 6502: Z flag is 0 |
| `Cond_set` | `set` | 6502: Z flag is 0 |
| `Cond_zero` | `zero` | 6502: Z flag is 1 |
| `Cond_unset` | `unset` | 6502: Z flag is 1 |
| `Cond_clear` | `clear` | 6502: Z flag is 1 |
| `Cond_equal` | `equal` | 6502: Z flag is 1 |
| `Cond_high` | `high` | 68000: HI condition code |
| `Cond_low_or_same` | `low_or_same` | 68000: LS condition code |
| `Cond_carry_clear` | `carry_clear` | 68000: CC condition code |
| `Cond_carry_set` | `carry_set` | 68000: CS condition code |
| `Cond_not_equal` | `not_equal` | 68000: NE condition code |
| `Cond_overflow_clear` | `overflow_clear` | 68000: VC condition code |
| `Cond_overflow_set` | `overflow_set` | 68000: VS condition code |
| `Cond_greater_or_equal` | `greater_or_equal` | 68000: GE condition code |
| `Cond_less_than` | `less_than` | 68000: LT condition code |
| `Cond_greater_than` | `greater_than` | 68000: GT condition code |
| `Cond_less_or_equal` | `less_or_equal` | 68000: LE condition code |
| `Cond_not_zero` | `not_zero` | Z80: Z flag is 0 |
| `Cond_no_carry` | `no_carry` | Z80: C flag is 0 |
| `Cond_parity_even` | `parity_even` | Z80: P/V flag is 1 |
| `Cond_parity_odd` | `parity_odd` | Z80: P/V flag is 0 |
| `Cond_sign_positive` | `sign_positive` | Z80: S flag is 0 |
| `Cond_sign_negative` | `sign_negative` | Z80: S flag is 1 |
| `Cond_true` | `true` | Shared: always true |
| `Cond_false` | `false` | Shared: always false |

#### Condition modifiers

| Token type | Source text | Meaning |
|------------|-------------|---------|
| `Mod_is` | `is` | Condition modifier (no-op) |
| `Mod_has` | `has` | Condition modifier (no-op) |
| `Mod_no` | `no` | Condition modifier (no-op) |
| `Mod_not` | `not` | Condition modifier (no-op) |

#### Mode prefixes

| Token type | Source text | Meaning |
|------------|-------------|---------|
| `Mode_zp` | `zp` | Zero page addressing mode |
| `Mode_abs` | `abs` | Absolute addressing mode |
| `Mode_rel` | `rel` | Relative addressing mode (branch) |
| `Mode_ind` | `ind` | Indirect addressing mode |
| `Mode_idx` | `idx` | Indexed addressing mode |
| `Mode_ind_l` | `ind_l` | Indirect long addressing mode |
| `Mode_ind_idx` | `ind_idx` | Indirect indexed addressing mode |

## Future work

These items are deferred. A future revision may define them.

1. A checksum field in the header for data integrity.
2. A compression flag in the header for compressed data blocks.
3. A debug information block for emulator debug support.
4. Cross-target `.opb` files that hold more than one target variant.