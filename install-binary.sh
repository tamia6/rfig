#!/bin/sh
set -eu

if [ "$(uname -s)" != Darwin ]; then
  printf 'rfig binaries currently support macOS only.\n' >&2
  exit 1
fi

version=v0.1.1
asset=rfig-macos-universal
base_url="https://github.com/tamia6/rfig/releases/download/$version"
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
scratch=$(mktemp -d)
trap 'rm -r "$scratch"' EXIT HUP INT TERM

find_asset() {
  name=$1
  for path in "$script_dir/$name" "$HOME/Downloads/$name"; do
    if [ -f "$path" ]; then
      printf '%s\n' "$path"
      return
    fi
  done
  curl -fsSL --retry 3 "$base_url/$name" -o "$scratch/$name"
  printf '%s\n' "$scratch/$name"
}

checksums=$(find_asset SHA256SUMS)
binary=''
for path in "$script_dir/$asset" "$HOME/Downloads/$asset" "$script_dir/rfig" "$HOME/Downloads/rfig"; do
  if [ -f "$path" ]; then
    binary=$path
    break
  fi
done
if [ -z "$binary" ]; then
  binary=$(find_asset "$asset")
fi
integration=$(find_asset rfig.zsh)

verify() {
  name=$1
  path=$2
  expected=$(awk -v name="$name" '$2 == name { print $1 }' "$checksums")
  actual=$(shasum -a 256 "$path" | awk '{ print $1 }')
  if [ -z "$expected" ] || [ "$actual" != "$expected" ]; then
    printf 'Checksum mismatch: %s\n' "$path" >&2
    exit 1
  fi
}

verify "$asset" "$binary"
verify rfig.zsh "$integration"

mkdir -p "$HOME/.local/bin" "$HOME/.config/rfig"
xattr -d com.apple.quarantine "$binary" 2>/dev/null || true
mv "$binary" "$HOME/.local/bin/rfig"
chmod 755 "$HOME/.local/bin/rfig"
cp "$integration" "$HOME/.config/rfig/rfig.zsh"

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
