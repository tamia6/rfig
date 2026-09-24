# rfig implementation plan

Goal: macOS zsh Tab completion with a compact cursor-anchored popup.

1. Build a Rust picker that accepts the current input line on stdin and the cursor offset as an argument. Return a selected full line and cursor offset to zsh; draw the popup directly on /dev/tty with crossterm.
2. Capture the current zsh completion candidates, including nested command contexts, and pass them to the Rust picker.
3. Bind a zsh widget to Tab. Tab accepts and continues, Enter accepts and closes, and unsupported inputs fall back to native completion.
4. Scan executable names in `$PATH` during installation and write an idempotent source line to ~/.zshrc.
5. Verify cargo test/build and drive a real zsh PTY through nested synthetic completions, `git checkout` branches, cancellation, and fallback.
