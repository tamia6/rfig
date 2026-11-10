# Crossterm and PowerShell validation — 2026-11-11

Base revision: `3726563`; results cover the uncommitted port. The final change after the macOS matrix defers ZLE resize handling only on Linux. Rust unit tests and Clippy were rerun on the final tree.

| Check | Result |
| --- | --- |
| Rust unit tests | 27/27 passed |
| Formatting and strict Clippy | Passed |
| macOS isolated shell suites | 35/36 passed; one known zsh slow-completer failure |
| Offline Docker matrix | 60/61 passed; the same known zsh slow-completer case |
| Installed macOS binary and IDEA's Reworked Terminal zsh startup scripts | Both double-ESC arrows selected candidates in an isolated tmux session; no `[A`/`[B` was inserted. Tab and Enter passed. |
| PowerShell setup and terminal interaction | Passed on macOS with pwsh 7.7.0: repeatable setup, registered completers, Alt arrows, quoted values, Unicode, execution and Ctrl+C. |
| Windows x64 | Linked a PE32+ console executable with a static C runtime and checked the ZIP. The Windows-native ConPTY test is configured in the release workflow; it was not run on this Mac. |

The prior [zsh baseline](../color-columns-2026-11-09/README.md) records the synchronous native-completer failure. In this revision the Rust editor continues to accept input, but zsh cannot finish returning to its prompt while a synchronous ZLE completer is running. The strict suite records the failure at the cancellation assertion. Linux prompt resizing, Unicode, and SIGTERM cleanup pass separately.

macOS 15.6.1 arm64: Rust 1.99.1, zsh 5.9, Bash 5.3.20, Fish 4.9.3, tmux 3.8. Docker `rust:1.91-bookworm` arm64: Rust 1.91.1, zsh 5.9, Bash 5.2.15 and 4.4.0, Fish 3.6.0, tmux 3.3a. Docker used the project bootstrap with Bash 4.4 enabled, then ran the full matrix serially with networking disconnected. The test container was removed.

Commands: `cargo test --locked`; `cargo fmt --check`; `cargo clippy --all-targets --all-features --locked -- -D warnings`; the isolated `tests/*.py` shell suites, including `tests/powershell_interaction.py`; and `python3 tests/docker/run_matrix.py` in the bootstrapped container. Windows build: `RUSTFLAGS='-C target-feature=+crt-static' cargo xwin build --release --locked --target x86_64-pc-windows-msvc`.

Windows ZIP SHA-256: `154434454f65153588a5ee1e136f516fe3b487626bfd89b9f0b5231cd518ecf4`. See [macOS logs](local-final/summary.json), [Docker logs](docker-final/summary.json), [Windows build log](windows-cross-build.log), and [IDEA smoke-test log](idea-zsh-installed.log).
