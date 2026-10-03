#!/bin/sh
set -eu

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
if [ -f "$project_dir/Cargo.toml" ]; then
  cargo install --path "$project_dir" --root "$HOME/.local" --force --locked
elif [ -f "$project_dir/rfig" ]; then
  case "$(uname -s)" in
    Darwin) expected=macos-universal ;;
    Linux)
      case "$(uname -m)" in
        x86_64|amd64) expected=linux-x86_64 ;;
        aarch64|arm64) expected=linux-aarch64 ;;
        *) printf 'Unsupported Linux architecture.\n' >&2; exit 1 ;;
      esac ;;
    *) printf 'rfig supports macOS and Linux.\n' >&2; exit 1 ;;
  esac
  # Older macOS bundles did not carry a target manifest.
  target=$(cat "$project_dir/TARGET" 2>/dev/null || printf macos-universal)
  if [ "$target" != "$expected" ]; then
    printf 'This bundle is for %s; this machine requires %s.\n' "$target" "$expected" >&2
    exit 1
  fi
  if command -v sha256sum >/dev/null 2>&1; then
    (cd "$project_dir" && sha256sum -c SHA256SUMS)
  else
    (cd "$project_dir" && shasum -a 256 -c SHA256SUMS)
  fi
  mkdir -p "$HOME/.local/bin"
  if [ "$expected" = macos-universal ]; then
    xattr -d com.apple.quarantine "$project_dir/rfig" 2>/dev/null || true
  fi
  mv "$project_dir/rfig" "$HOME/.local/bin/rfig"
  chmod 755 "$HOME/.local/bin/rfig"
else
  printf 'rfig source or binary is missing next to install.sh.\n' >&2
  exit 1
fi

# Integration scripts are embedded in the binary. setup writes the selected adapter.
PATH="$HOME/.local/bin:$PATH" "$HOME/.local/bin/rfig" setup "$@"
