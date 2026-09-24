#!/bin/sh
set -eu

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
cargo install --path "$project_dir" --root "$HOME/.local" --force --locked
mkdir -p "$HOME/.config/rfig"
cp "$project_dir/rfig.zsh" "$HOME/.config/rfig/rfig.zsh"
"$HOME/.local/bin/rfig" setup
