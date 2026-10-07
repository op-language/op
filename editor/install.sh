#!/usr/bin/env sh
# Install the Op editor support files into the user's Neovim config.
#
# Installs:
#   - the ftdetect file into $NVIM_CONFIG/ftdetect/op.lua
#   - the regex fallback file into $NVIM_CONFIG/syntax/op.vim
#
# Set XDG_CONFIG_HOME to select another config root. The tree-sitter
# parser install is a manual step; see tree-sitter-op/README.md.

set -eu

# Resolve the editor directory relative to this script.
SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
EDITOR_DIR="$SCRIPT_DIR"

NVIM_CONFIG="${XDG_CONFIG_HOME:-$HOME/.config}/nvim"

install -d "$NVIM_CONFIG/ftdetect"
install -d "$NVIM_CONFIG/syntax"
install -m 0644 "$EDITOR_DIR/ftdetect/op.lua" "$NVIM_CONFIG/ftdetect/op.lua"
install -m 0644 "$EDITOR_DIR/syntax/op.vim" "$NVIM_CONFIG/syntax/op.vim"

echo "Op editor support installed:"
echo "  ftdetect: $NVIM_CONFIG/ftdetect/op.lua"
echo "  syntax:   $NVIM_CONFIG/syntax/op.vim"