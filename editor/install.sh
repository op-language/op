#!/usr/bin/env sh
# Install the Op editor support into the user's Neovim config.
#
# Installs:
#   - the tree-sitter parser (op.so) and query files into
#     ~/.local/share/nvim/site/
#   - the ftdetect and regex-syntax fallback files into
#     ~/.config/nvim/
#
# Run from the op/editor/ directory or the repo root. Requires the
# tree-sitter CLI on PATH (for the parser build).

set -eu

# Resolve the editor directory relative to this script.
SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
EDITOR_DIR="$SCRIPT_DIR"
TS_DIR="$EDITOR_DIR/tree-sitter-op"

# 1. Build and install the tree-sitter parser and queries.
#    The Makefile handles generate, build, and install of op.so + *.scm
#    into ~/.local/share/nvim/site/.
make -C "$TS_DIR" install

# 2. Install the ftdetect and regex-syntax fallback files.
NVIM_CONFIG="${XDG_CONFIG_HOME:-$HOME/.config}/nvim"
install -d "$NVIM_CONFIG/ftdetect"
install -d "$NVIM_CONFIG/syntax"
install -m 0644 "$EDITOR_DIR/ftdetect/op.lua" "$NVIM_CONFIG/ftdetect/op.lua"
install -m 0644 "$EDITOR_DIR/syntax/op.vim" "$NVIM_CONFIG/syntax/op.vim"

echo "Op editor support installed:"
echo "  parser:   ~/.local/share/nvim/site/parser/op.so"
echo "  queries:  ~/.local/share/nvim/site/queries/op/"
echo "  ftdetect: $NVIM_CONFIG/ftdetect/op.lua"
echo "  syntax:   $NVIM_CONFIG/syntax/op.vim"