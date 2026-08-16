# tree-sitter-op

A tree-sitter grammar for the [Op](https://github.com/op-language/op)
programming language — a high-level assembler for retro game consoles and
home computers.

## Prerequisites

- [tree-sitter CLI](https://github.com/tree-sitter/tree-sitter) 0.26+
  (`npm install -g tree-sitter-cli`)
- A C compiler (`cc` / `gcc` / `clang`)

## Build and install

Run the Makefile to generate the parser, compile the shared object, and
install it into Neovim's site directory:

```sh
make all
```

This copies `op.so` to `~/.local/share/nvim/site/parser/op.so` and the
query files to `~/.local/share/nvim/site/queries/op/`.

To install all editor support (tree-sitter parser, query files, ftdetect,
and the regex syntax fallback) in one command, run the editor install
script from the repo root:

```sh
./editor/install.sh
```

This wraps `make install` and also copies `ftdetect/op.lua` and
`syntax/op.vim` into `~/.config/nvim/`.

## Targets

| Target | Description |
|--------|-------------|
| `make generate` | Run `tree-sitter generate` to produce `src/parser.c`. |
| `make build` | Compile the parser to `op.so`. |
| `make install` | Copy `op.so` and query files into Neovim's site directory. |
| `make test` | Run `tree-sitter parse` on the corpus files. |
| `make clean` | Remove build artifacts. |
| `make all` | `generate` + `build` + `install`. |

## File types

- Extension: `.op`
- Filetype: `op`
- Tree-sitter scope: `source.op`

## License

Apache-2.0, same as the Op language project.