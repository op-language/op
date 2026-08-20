# Op Library Binary (.opb) File Format

Version 1.0

This document defines the `.opb` file format. The `cart build` command
invokes the `opc` compiler to build a lib project. The `opc` compiler
writes the `.opb` file to `target/<triplet>/<libname>.opb`. The linker
reads the `.opb` file when a ROM project depends on the lib.

This document uses the keywords **must**, **shall**, and **may** as RFC
2119 defines.

## Scope

This document defines the binary layout of the `.opb` file, the JSON
layout of the `.opx` file, the JSON layout of the `.opa` file, and the
JSON layout of the `.opl` file. For the `.opb` file, the document defines
the header, the section table, the symbol table, the relocation table,
and the data blocks. For the `.opx` file, the document defines the
envelope fields, the token fields, and the token type reference table.
For the `.opa` file, the document defines the envelope fields, the module
fields, the item node types, the function body statement node types, the
expression node types, and the operand node types. For the `.opl` file,
the document defines the envelope fields, the section fields, the symbol
fields, the relocation fields, and the relocation kind values.

This document does not define the Op language grammar. The document
`language-specification.md` defines the grammar.

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

## Op AST (.opa) File Format

Version 1.0

### Purpose

The `.opa` file is the JSON output of the `opc --parse` stage. The file
holds the abstract syntax tree (AST) that the codegen stage reads. A
developer can inspect the `.opa` file to debug the parser output.

### Format

The `.opa` file is a JSON document. The file uses UTF-8 encoding. The
`opc` binary writes the file with pretty-printed JSON (two-space
indentation).

### Envelope fields

The top-level JSON object has three fields.

| Field | Type | Description |
|-------|------|-------------|
| `version` | integer | The format version. Current value: `1`. |
| `target` | string | The target triplet string. |
| `root` | object | The root Module object. |

### Module fields

The `root` object represents the root module of the program.

| Field | Type | Description |
|-------|------|-------------|
| `kind` | string | Always `"Module"`. |
| `name` | string | The module name. |
| `items` | array | The list of Item objects in this module. |

### Item node types

Each element of the `items` array is a JSON object with a `kind` field
that identifies the item type. The table below lists every item type and
its fields.

| Item kind | Fields | Description |
|-----------|--------|-------------|
| `ConstDecl` | `name`, `ty`, `value`, `evaluated_value`, `attributes` | A compile-time constant. `evaluated_value` is `null` if the evaluator could not compute the value. |
| `VarDecl` | `name`, `is_volatile`, `ty`, `array_dim`, `addr_binding`, `init`, `attributes` | A variable declaration. `array_dim`, `addr_binding`, and `init` are `null` when absent. |
| `FnDecl` | `name`, `is_noreturn`, `attributes`, `body` | A function declaration. `body` is an array of FnStmt objects. |
| `InlineFnDecl` | `name`, `params`, `attributes`, `body` | An inline macro function. `params` is an array of strings. |
| `StructDecl` | `name`, `fields`, `attributes` | A struct type. `fields` is an array of Field objects. |
| `TypeDecl` | `name`, `ty`, `attributes` | A type alias. |
| `EnumDecl` | `name`, `variants`, `attributes` | An enum declaration. `variants` is an array of EnumVariant objects. |
| `ModDecl` | `name`, `is_pub`, `body`, `resolved`, `attributes` | A module declaration. `body` is `null` for a file module. `resolved` is `null` if the sub-module file was not found. |
| `UseDecl` | `is_pub`, `trees` | A use declaration. `trees` is an array of UseTree objects. |
| `BlockAttribute` | `attr`, `items` | A block attribute such as `#[rom(...)] { ... }`. |
| `Placement` | `macro_name`, `argument`, `attributes` | A placement macro call such as `locate_fn!(path::name)`. |

### Attribute object

An `Attribute` object has two fields.

| Field | Type | Description |
|-------|------|-------------|
| `path` | string | The attribute path (e.g. `"cfg"`, `"rom"`, `"interrupt"`). |
| `args` | array | The list of AttrArg objects. |

An `AttrArg` object has two fields.

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | The argument name. Empty for positional arguments. |
| `value` | string | The argument value as a string. |

### Field object

A `Field` object represents a struct field.

| Field | Type | Description |
|-------|------|-------------|
| `is_volatile` | boolean | True if the field is volatile. |
| `name` | string | The field name. |
| `ty` | object | The field type (a Type object). |
| `array_dim` | object or null | The array dimension expression, or `null`. |

### EnumVariant object

An `EnumVariant` object represents an enum variant.

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | The variant name. |
| `value` | object or null | The variant value expression, or `null`. |

### Type object

A `Type` object has a `kind` field that identifies the type form.

| Type kind | Fields | Description |
|-----------|--------|-------------|
| `Named` | `name` | A named type (identifier or primitive). |
| `Array` | `element`, `size` | An array type. `element` is a Type object. `size` is an Expr object or `null`. |

### UseTree object

A `UseTree` object has a `kind` field that identifies the form.

| UseTree kind | Fields | Description |
|--------------|--------|-------------|
| `Path` | `root`, `segments`, `tail` | A path import. `root` is a UseRoot object. `segments` is an array of strings. `tail` is a UseTail object. |
| `Alias` | `inner`, `alias` | A path import with an alias. `inner` is a UseTree object. `alias` is a string. |

### UseRoot object

A `UseRoot` object has a `kind` field.

| UseRoot kind | Fields | Description |
|--------------|--------|-------------|
| `Lib` | none | The `lib::` path root. |
| `SelfMod` | none | The `self::` path root. |
| `Super` | none | The `super::` path root. |
| `Name` | `name` | A named path root. |

### UseTail object

A `UseTail` object has a `kind` field.

| UseTail kind | Fields | Description |
|--------------|--------|-------------|
| `Item` | none | The last segment is an item name. |
| `Glob` | none | A glob import `::*`. |
| `Group` | `group` | A group import `::{a, b, c}`. `group` is an array of UseTree objects. |

### Function body statement node types

Each element of a `body` array is a FnStmt object with a `kind` field.

| FnStmt kind | Fields | Description |
|-------------|--------|-------------|
| `Label` | `name`, `stmt` | A label definition. `stmt` is the FnStmt that follows the label. |
| `AsmStmt` | `opcode`, `operands` | An assembly statement. `opcode` is the mnemonic string. `operands` is an array of Operand objects. |
| `IfStmt` | `branch_hint`, `condition`, `then_block`, `else_block` | An if statement. `branch_hint` is `"Near"` or `"Far"` or `null`. `condition` is a Condition object. `then_block` is an array of FnStmt objects. `else_block` is an array or `null`. |
| `WhileStmt` | `branch_hint`, `condition`, `body` | A while loop. |
| `DoWhileStmt` | `body`, `branch_hint`, `condition` | A do-while loop. |
| `LoopStmt` | `body` | An endless loop. |
| `SwitchStmt` | `register`, `cases` | A switch statement. `register` is the register name string. `cases` is an array of SwitchCase objects. |
| `FnCall` | `name`, `args` | A function or macro call. `args` is an array of Expr objects. |
| `ReturnStmt` | none | A return statement. |
| `VarDeclStmt` | `decl` | A variable declaration inside a function body. `decl` is a VarDecl Item object. |

### Condition object

A `Condition` object has two fields.

| Field | Type | Description |
|-------|------|-------------|
| `modifiers` | array of strings | The modifier keywords (e.g. `["is"]`). |
| `keyword` | string | The condition keyword (e.g. `"plus"`, `"carry"`). |

### SwitchCase object

A `SwitchCase` object has a `kind` field.

| SwitchCase kind | Fields | Description |
|-----------------|--------|-------------|
| `Case` | `expr`, `body` | A case block. `expr` is an Expr object. `body` is an array of FnStmt objects. |
| `Default` | `body` | The default block. |

### Operand node types

Each element of an `operands` array is an Operand object with a `kind`
field.

| Operand kind | Fields | Description |
|--------------|--------|-------------|
| `Immediate` | `value` | An immediate operand `#expr`. `value` is an Expr object. |
| `MemoryOperand` | `mode_prefix`, `expr`, `index_reg` | A memory operand. `mode_prefix` is a string or `null`. `expr` is an Expr object. `index_reg` is a string or `null`. |
| `RegisterRef` | `name` | A register reference `cpu::ident`. |
| `LabelRef` | `name` | A label reference `'ident`. |
| `Selector` | `path`, `accesses` | A selector expression. `path` is an array of strings. `accesses` is an array of Access objects. |

### Expression node types

An `Expr` object has a `kind` field that identifies the expression form.

| Expr kind | Fields | Description |
|-----------|--------|-------------|
| `Number` | `value` | An integer literal. `value` is a 64-bit integer. |
| `String_` | `value` | A string literal. `value` is the string content. |
| `Boolean` | `value` | A boolean literal. `value` is `true` or `false`. |
| `Ident` | `name` | A bare identifier. |
| `BinOp` | `op`, `left`, `right` | A binary operation. `op` is a BinaryOp string. `left` and `right` are Expr objects. |
| `UnaryOp` | `op`, `operand` | A unary operation. `op` is a UnaryOp string. `operand` is an Expr object. |
| `MacroCall` | `name`, `arg` | A compile-time macro call. `name` is the macro name. `arg` is an Expr object. |
| `Selector` | `path`, `accesses` | A selector expression. `path` is an array of strings. `accesses` is an array of Access objects. |
| `FnCall` | `name`, `args` | A function call expression. `args` is an array of Expr objects. |
| `ParenExpr` | `inner` | A parenthesized expression. `inner` is an Expr object. |

### BinaryOp values

| Value | Operator |
|-------|----------|
| `Or` | `\|` |
| `Xor` | `^` |
| `And` | `&` |
| `Eq` | `==` |
| `Ne` | `!=` |
| `Lt` | `<` |
| `Gt` | `>` |
| `Le` | `<=` |
| `Ge` | `>=` |
| `Shl` | `<<` |
| `Shr` | `>>` |
| `Add` | `+` |
| `Sub` | `-` |
| `Mul` | `*` |
| `Div` | `/` |
| `Mod` | `%` |

### UnaryOp values

| Value | Operator |
|-------|----------|
| `Not` | `!` |
| `Inv` | `~` |
| `Neg` | `-` (unary) |
| `Pos` | `+` (unary) |

### Access object

An `Access` object has a `kind` field.

| Access kind | Fields | Description |
|-------------|--------|-------------|
| `ModuleAccess` | `name` | A `::ident` access. |
| `FieldAccess` | `name` | A `.ident` access. |
| `Offset` | `op`, `value` | A `+expr` or `-expr` offset. `op` is `"Add"` or `"Sub"`. `value` is an Expr object. |

### InitValue object

An `InitValue` object has a `kind` field.

| InitValue kind | Fields | Description |
|----------------|--------|-------------|
| `Expr` | `value` | A single expression initializer. |
| `InitList` | `items` | A brace-enclosed list. `items` is an array of InitValue objects. |
| `String_` | `value` | A string literal initializer. |

### PlacementArg object

A `PlacementArg` object has a `kind` field.

| PlacementArg kind | Fields | Description |
|-------------------|--------|-------------|
| `String_` | `value` | A string literal argument. |
| `Path` | `segments` | A module path argument. `segments` is an array of strings. |

### Example

Source file `main.op`:

```
const SCREEN_WIDTH: u8 = 256;

fn main() {
    lda 0
    sta PPU::CNT0
}
```

`.opa` output:

```json
{
  "version": 1,
  "target": "mos6502-nintendo-nes-ntsc",
  "root": {
    "kind": "Module",
    "name": "main",
    "items": [
      {
        "kind": "ConstDecl",
        "name": "SCREEN_WIDTH",
        "ty": { "kind": "Named", "name": "u8" },
        "value": { "kind": "Number", "value": 256 },
        "evaluated_value": 256,
        "attributes": []
      },
      {
        "kind": "FnDecl",
        "name": "main",
        "is_noreturn": false,
        "attributes": [],
        "body": [
          {
            "kind": "AsmStmt",
            "opcode": "lda",
            "operands": [
              { "kind": "Selector", "path": [], "accesses": [] }
            ]
          },
          {
            "kind": "AsmStmt",
            "opcode": "sta",
            "operands": [
              {
                "kind": "Selector",
                "path": ["PPU"],
                "accesses": [
                  { "kind": "ModuleAccess", "name": "CNT0" }
                ]
              }
            ]
          }
        ]
      }
    ]
  }
}
```

## Op Object Data (.opl) File Format

Version 1.0

### Purpose

The `.opl` file is the JSON output of the `opc --compile` stage and the
`opc --link` stage. The post-compile file holds the object data before
the linker resolves relocations. The post-link file holds the final
object data after the linker resolves relocations and lays out the
sections. A developer can inspect the `.opl` file to debug the codegen
and linker output.

### Format

The `.opl` file is a JSON document. The file uses UTF-8 encoding. The
`opc` binary writes the file with pretty-printed JSON (two-space
indentation).

### Envelope fields

The top-level JSON object has these fields.

| Field | Type | Description |
|-------|------|-------------|
| `version` | integer | The format version. Current value: `1`. |
| `target` | string | The target triplet string. |
| `sections` | array | The list of Section objects. |
| `interrupt_vectors` | array | The list of InterruptVector objects. The codegen records these from `#[interrupt(name)]` attributes. The linker writes the vector table entries into the ROM data. |
| `header` | object or null | The HeaderFields object from `#[ines(...)]` or `#[lnx(...)]` attributes. The file output stage reads this to write the output file header. |
| `pad_byte` | integer | The padding byte from `#[setpad(value)]`. The linker pads ROM and CHR sections to their `maxsize` with this byte. Default value: `0`. |

### Section fields

Each element of the `sections` array is a JSON object with these fields.

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | The section name. |
| `kind` | string | The section kind. Valid values: `rom`, `ram`, `chr`. |
| `org` | integer | The origin address of the section. |
| `bank` | integer | The bank number of the section. |
| `maxsize` | integer | The maximum byte count of the section. |
| `symbols` | array | The list of Symbol objects in this section. |
| `relocations` | array | The list of Relocation objects in this section. |
| `data` | array of integers | The raw byte data. Each integer is a byte value from 0 to 255. |

### Symbol fields

Each element of the `symbols` array is a JSON object with these fields.

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | The symbol name. |
| `offset` | integer | The offset of the symbol from the section origin. |
| `size` | integer | The byte size of the symbol. |
| `kind` | string | The symbol kind. Valid values: `function`, `variable`, `label`. |
| `is_pub` | boolean | True if the symbol is public and visible across lib boundaries. |

### Relocation fields

Each element of the `relocations` array is a JSON object with these
fields.

| Field | Type | Description |
|-------|------|-------------|
| `offset` | integer | The byte offset within the section data where the linker must patch the value. |
| `kind` | string | The relocation kind. See the table below. |
| `symbol` | string | The name of the symbol that the relocation references. |

### Relocation kind values

| Value | Size | Description |
|-------|------|-------------|
| `abs8` | 1 byte | Absolute 8-bit address. |
| `abs16` | 2 bytes | Absolute 16-bit address. |
| `abs24` | 3 bytes | Absolute 24-bit address (65C816). |
| `abs32` | 4 bytes | Absolute 32-bit address (68000). |
| `branch8` | 1 byte | Relative branch offset, 8-bit signed. |
| `branch16` | 2 bytes | Relative branch offset, 16-bit signed. |
| `lo8` | 1 byte | Low byte of a 16-bit symbol address. |
| `hi8` | 1 byte | High byte of a 16-bit symbol address. |
| `bank` | 1 byte | Bank number of a symbol (Lynx). |

### InterruptVector fields

Each element of the `interrupt_vectors` array is a JSON object with these
fields.

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | The interrupt name. Valid values: `reset`, `nmi`, `irq`. |
| `address` | integer | The vector table address where the linker writes the 2-byte target. For the 6502, `reset` is `0xFFFC`, `nmi` is `0xFFFA`, `irq` is `0xFFFE`. |
| `target` | string | The symbol name of the target function. |

### HeaderFields fields

The `header` field is a JSON object with these fields.

| Field | Type | Description |
|-------|------|-------------|
| `format` | string | The format name. Valid values: `ines`, `lnx`, `sega`, `snes`, `gb`, `sms`, `a78`. |
| `fields` | array of pairs | The key-value pairs from the attribute arguments. Each pair is a two-element array of strings. |

### Post-compile and post-link differences

The post-compile `.opl` file has unresolved relocations. The `data`
array holds the encoded bytes with placeholder values where the linker
must patch relocations. The `relocations` array lists every relocation
that the linker must resolve. The `interrupt_vectors`, `header`, and
`pad_byte` fields on the `ObjectFile` carry the metadata that the codegen
recorded from the source attributes.

The post-link `.opl` file has all relocations resolved. The `data` array
holds the final bytes with all relocation sites patched. The
`relocations` array is empty. The linker writes the interrupt vector
table entries into the ROM section data at the vector table addresses.
The linker pads each ROM and CHR section to its `maxsize` with the
`pad_byte` value. The linker does not pad RAM sections.

The `interrupt_vectors` and `header` fields on the `ObjectFile` pass
through the linker to the file output stage. The file output stage reads
these fields to write the output file header.

### Example

Source file `main.op`:

```
#[rom(org = 0xC000, bank = 0, maxsize = 0x4000)] {
    fn main() {
        lda 0
        sta 0x2000
    }
}
```

Post-compile `.opl` output:

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
        { "name": "main", "offset": 0, "size": 6, "kind": "function", "is_pub": false }
      ],
      "relocations": [
        { "offset": 3, "kind": "abs16", "symbol": "0x2000" }
      ],
      "data": [ 169, 0, 141, 0, 0, 96 ]
    }
  ],
  "interrupt_vectors": [],
  "header": null,
  "pad_byte": 0
}
```

Post-link `.opl` output for the same source. The `relocations` array is
empty. The `data` array holds the patched bytes. The section is padded
to `maxsize`. The `interrupt_vectors`, `header`, and `pad_byte` fields
pass through to the file output stage.

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
        { "name": "main", "offset": 0, "size": 6, "kind": "function", "is_pub": false }
      ],
      "relocations": [],
      "data": [ 169, 0, 141, 0, 32, 96, 255, 255, 255, "...", 255, 0, 192 ]
    }
  ],
  "interrupt_vectors": [
    { "name": "reset", "address": 65532, "target": "main" }
  ],
  "header": null,
  "pad_byte": 255
}
```

The vector table entry for `reset` writes the 2-byte address `0xC000`
into the ROM data at offset `0x3FFC` (`0xFFFC - 0xC000`). The `data`
array shows `0, 192` at that offset, which is `0xC000` in little-endian
order. The section is padded to `maxsize` (`16384` bytes) with the
`pad_byte` value `255`.

## Future work

These items are deferred. A future revision may define them.

1. A checksum field in the header for data integrity.
2. A compression flag in the header for compressed data blocks.
3. A debug information block for emulator debug support.
4. Cross-target `.opb` files that hold more than one target variant.