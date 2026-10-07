# Op language support for Neovim

This folder holds the editor support for the Op language.

- `ftdetect/op.lua` — sets the `op` filetype for `*.op` files.
- `syntax/op.vim` — the complete regex fallback for the Op language.
- `install.sh` / `uninstall.sh` — install and remove the two files.
- `tree-sitter-op/` — the tree-sitter grammar. It is the primary highlighter when you install its parser and queries.

## Install

```sh
./install.sh
```

The script copies `ftdetect/op.lua` and `syntax/op.vim` into `${XDG_CONFIG_HOME:-$HOME/.config}/nvim`. It creates the target directories when they do not exist. Set `XDG_CONFIG_HOME` to select another config root. Run the script again at any time. A second run writes the same files with the same result.

To remove the files, run:

```sh
./uninstall.sh
```

The uninstall script removes only the two named files.

## Coverage

The highlighter shows the full token surface of the current compiler. It covers the language keywords, the primitive types, the 31 condition words and the four modifiers, the addressing-mode prefixes, the compile-time and include macros, the attributes, labels, registers, decimal, binary, and hexadecimal literals, strings with their escape set, comments, and every machine-specific opcode of these CPU families:

- 6502 core and the 6502 undocumented set
- 65SC02
- W65C02 with the Rockwell bit operations
- 65C816, plus the nine mnemonics the standard CPU library declares (`phb phd phk plb pld tad tda tsa wdm`)
- 68000
- Z80
- SM83/LR35902, with the official forms and the 57 underscore register-pair pseudo-instructions

## Source of truth

`crates/opc/src/lexer.rs` holds the opcode vocabulary (the `OPCODES` const, lines 143 to 177) and the word tables for the other token groups. When the lexer changes, update `syntax/op.vim` in the same change. The file header repeats these references.

The tree-sitter grammar in `tree-sitter-op/` shares this vocabulary.

## Note on the NvChad config

The target config is NvChad v2.5 with lazy.nvim. lazy.nvim filters some runtime scripts. Neovim loads `syntax/op.vim` and `ftdetect/*.lua` through its own loaders, so that filter does not affect the Op files at the paths above.

## The tree-sitter parser

The regex fallback is the actual highlighter on this machine, because no `op` parser is installed. To use the grammar highlighter instead, build and install the parser manually:

```sh
make -C tree-sitter-op install
```

When the parser is attached, the tree-sitter queries take over the highlighting.

## License

This folder carries the license of the repository: Apache-2.0. See the `LICENSE` file at the repository root.