#!/bin/sh
set -eu

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
if [ -f "$project_dir/Cargo.toml" ]; then
  cargo install --path "$project_dir" --root "$HOME/.local" --force --locked
elif [ -f "$project_dir/rfig" ]; then
  if [ "$(uname -s)" != Darwin ]; then
    printf 'The prebuilt rfig binary currently supports macOS only.\n' >&2
    exit 1
  fi
  (cd "$project_dir" && shasum -a 256 -c SHA256SUMS)
  mkdir -p "$HOME/.local/bin"
  xattr -d com.apple.quarantine "$project_dir/rfig" 2>/dev/null || true
  mv "$project_dir/rfig" "$HOME/.local/bin/rfig"
  chmod 755 "$HOME/.local/bin/rfig"
else
  printf 'rfig source or binary is missing next to install.sh.\n' >&2
  exit 1
fi

mkdir -p "$HOME/.config/rfig"
cp "$project_dir/rfig.zsh" "$HOME/.config/rfig/rfig.zsh"

case ":$PATH:" in
  *":$HOME/.local/bin:"*) ;;
  *)
    rc_dir=${ZDOTDIR:-$HOME}
    mkdir -p "$rc_dir"
    rc="$rc_dir/.zshrc"
    path_line='export PATH="$HOME/.local/bin:$PATH"'
    if [ ! -f "$rc" ] || ! grep -Fqx "$path_line" "$rc"; then
      printf '%s\n' "$path_line" >> "$rc"
    fi
    ;;
esac

PATH="$HOME/.local/bin:$PATH" "$HOME/.local/bin/rfig" setup
