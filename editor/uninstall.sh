#!/usr/bin/env sh
# Uninstall the Op editor support files from the user's Neovim config.
#
# Removes exactly two files and touches no other file:
#   - $NVIM_CONFIG/ftdetect/op.lua
#   - $NVIM_CONFIG/syntax/op.vim
#
# Set XDG_CONFIG_HOME to select another config root. A run before an
# install removes nothing and still succeeds.

set -eu

NVIM_CONFIG="${XDG_CONFIG_HOME:-$HOME/.config}/nvim"

for file in "$NVIM_CONFIG/ftdetect/op.lua" "$NVIM_CONFIG/syntax/op.vim"; do
  if [ -f "$file" ]; then
    rm -f "$file"
    echo "removed: $file"
  else
    echo "not present: $file"
  fi
done