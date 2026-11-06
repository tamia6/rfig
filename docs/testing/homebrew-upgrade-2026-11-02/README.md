# Homebrew upgrade verification — 2026-11-02

The running Homebrew executable is recognized by its canonical Cellar path and equality with opt/rfig/bin/rfig. Setup atomically links each configured adapter to opt/rfig/share/rfig/rfig.<shell>. Other installs atomically replace the file, preserving package files when switching away from Homebrew. Existing shell sessions must restart; startup runs no extra process or scan.

## Results

- Rust: 17 unit tests pass.
- Local full matrix: **30/31 suites pass**. Bash 4.4 (built from checksum-verified GNU source), Fish 4.9.3 and zsh 5.9. Source flags for Bash 4.4 were adjusted for current Apple Clang, without patching Bash source.
- Docker full matrix: **52/53 suites pass**. Debian Bookworm, Bash 5.2 and 4.4, Fish 3.6, zsh 5.9; unprivileged, sequential, offline test stage, 2 CPU / 2 GiB / 256 PIDs. Container removed at completion.
- Only remaining failed suite in either matrix is the previously recorded zsh slow native completion test (`test_slow_provider_input_latency_and_cancel`). Failure matches the baseline; a deliberately slow native provider blocks ZLE. The separate-editor SIGTERM case is skipped for zsh because it uses ZLE instead.
- Initial local Bash interaction attempts failed because its temporary executable was not yet built. Those logs are kept under local/preflight. After resolving the compiler compatibility flags, all six affected Bash suites were rerun and passed; unrelated passing suites were not repeated.
- The new Homebrew fixture passes locally and in Docker: all three shells actually source adapters before and after opt switches, without setup after upgrading. Covers paths with spaces, duplicate-free setup, legacy startup lines, history preservation, inactive keg, missing package script fallback, and migration to standalone without rewriting package scripts.
- The old implementation failed the new fixture before production edits; see red-test.txt.
- Actual local ~/.zshrc was exercised in a private tmux server. It resolves /opt/homebrew/bin/rfig and displays history first with ◷; Right inserts explicitly, Enter executes only actual input, and the menu clears. Verification history/usage hooks were disabled in that private shell so personal records were not changed.
- English/Chinese README and website describe migration and restarts. Website HTML parses and inline JavaScript passes node --check. No publication performed.

## Local migration

Backed up the standalone executable and configured zsh adapter under ~/.config/rfig/homebrew-entry-backup-20261102-m2wrzgt4/, removed ~/.local/bin/rfig, and linked ~/.config/rfig/rfig.zsh to /opt/homebrew/opt/rfig/share/rfig/rfig.zsh. Homebrew remains at the published v0.3.0; no package-owned executable was manually replaced. Only the already configured zsh was migrated; Bash/Fish were not enabled in the user's startup files. Existing history and usage files were preserved. The new setup implementation remains local until a future release.

## Reproduction

`cargo test --locked -j 2`, `python3 tests/homebrew_upgrade.py`, `python3 docs/testing/homebrew-upgrade-2026-11-02/local/run.py` from the repository root (or the saved driver path), and `RFIG_RESULTS_DIR=<directory> sh tests/docker/run.sh`. The local driver records each suite and targets the temporary Bash path shown in local/run.py. Full logs and final summaries are in local/ and docker/; source identities are in tested-source.sha256.
