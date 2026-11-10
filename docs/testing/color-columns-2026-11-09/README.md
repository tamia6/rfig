# Colored candidate columns — 2026-11-09

Restores numbered candidate rows with aligned icon, label, and description columns in zsh and the shared Bash/Fish editor. Inline suffixes remain gray. Category colors and bold selection distinguish menu rows. Visible shortcut numbers are relative to the displayed five-row window.

Command/Alt selection, Tab insertion, Enter execution, and plain-arrow behavior are preserved. A real tmux screen test checks separate candidate rows and color while the inline suffix stays gray. The local zsh adapter matches the source, with a backup at `~/.config/rfig/rfig.zsh.before-color-columns-20261109`. The release binary is installed at `~/.local/bin/rfig`.

Commands: `cargo test --locked`, `cargo build --release --locked`, the Python suites recorded in `local/summary.json`, and `tests/docker/run_matrix.py` in the prepared Rust 1.91 Debian container, offline, non-root, with init and resource limits. The initial in-progress run was superseded after fixing repeated zsh local declarations; only `docker-final` is acceptance evidence.

The zsh slow-provider latency failure is the existing synchronous-provider baseline documented in `../inline-command-keys-2026-11-08/README.md`. Zsh skips the separate-editor SIGTERM test because it uses ZLE. All other observed failures must be investigated before completion.

README and local English/Chinese website source were updated; no website publication or Git push was performed. GUI Terminal.app rendering was not directly observed; color and selection were verified through tmux/PTY.

## Final results

- Rust: 22/22 unit tests passed.
- Local: 33/34 suites passed; only `zsh-strict_shell` fails the existing slow-provider latency case.
- Docker: 60/61 suites passed; only `zsh59-strict` fails the same baseline case.
- Menu display, category color, inline gray suffix, selection, insertion, execution, prompt geometry, and other shell suites passed.
- Test containers, temporary test image, and manual tmux probe were removed. No matching test editor/provider processes remained after the suites finished.
