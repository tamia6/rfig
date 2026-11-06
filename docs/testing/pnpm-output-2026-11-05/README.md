# Completion output isolation — 2026-11-05

## Cause and fix

The pnpm fallback launcher prints its native-install warning when stderr is attached to a terminal. zsh's synchronous native completion inherited stdout/stderr, so each keystroke could print the warning into the edited line and leave ZLE's cursor/display state out of sync. The same problem applies to any noisy completion provider. Generated zsh scripts could also print startup output.

- Suppress stdout/stderr while zsh gathers native candidates through compadd and loads generated definitions. Candidate capture remains in the same shell, preserving completion state and insertion fields.
- Keep stderr separate from stdout for runtime completion queries. Help analysis still accepts stderr because many programs print help there. Dynamic protocol diagnostics are no longer parsed as candidate rows.
- Treat Bash's generic `_minimal` and `_comp_complete_minimal` registrations as fallbacks when a generated command definition exists. The first Debian regression exposed that these placeholders prevented the generated definition from loading. Genuine native definitions retain priority.
- Normal explicit command execution keeps its stdout/stderr unchanged. No branches match pnpm or another user command name.

## Results

- Rust: **17 unit tests pass**.
- Local full regression: **33/34 suites pass** (Bash 4.4, Fish 4.9.3, zsh 5.9).
- Docker full regression: **56/57 suites pass** (Debian Bookworm; Bash 5.2 and 4.4, Fish 3.6, zsh 5.9; sequential, non-root, offline test phase; 2 CPUs, 2 GiB, 256 PIDs).
- All new completion-output cases pass: native diagnostics/direct stdout, generated-script startup diagnostics, protocol stderr exclusion, real-candidate insertion and explicit-execution diagnostics. Bash checks both generic placeholder function names.
- Only remaining failed suite in each matrix is the previously documented zsh slow native completion case, `test_slow_provider_input_latency_and_cancel`, matching the Homebrew-upgrade baseline. The zsh separate-editor SIGTERM test remains inapplicable/skipped because zsh uses ZLE.
- The initial Docker run exposed the Bash placeholder bug; it was fixed and both full matrices rerun. Initial logs and red tests are under preflight/.
- A local Fish history assertion sampled an in-progress terminal redraw: its second diagnostic capture already contained the expected input. Changed that assertion to wait for the unchanged input rather than reading a single arbitrary frame. Reran the affected history suite in all three local shells; all passed. The updated fixture was copied into the preparing Docker workspace before its matrix ran. This changes test synchronization only.

## pnpm reproduction

The installed Homebrew pnpm is 12.16.1. The package source contains the two warning lines behind `test -t 2`; the local active executable does not currently emit this warning by itself. In isolated HOME, wrapped the unchanged real executable with those two TTY-conditional warning lines and used its shipped zsh completion definition. No installation/build scripts or pnpm configuration were changed.

The pre-fix adapter reproduced repeated warning lines and corrupted input (preflight/pnpm-red.log and local/red-real/). The fixed adapter shows a clean menu, supports Down then Right insertion, and clears on cancel. Running pnpm --version explicitly still displays the warning (local/real-pnpm.log). local/green-real/clean-menu.screen.txt captures the clean menu before explicit execution.

## Commands and artifacts

Run `cargo test --locked -j 2`; use RFIG_TEST_SHELL and RFIG_TEST_SHELL_EXE with `python3 tests/completion_output.py`; run the saved local/run.py from the repository root; and run `RFIG_RESULTS_DIR=<directory> sh tests/docker/run.sh` for the complete Docker matrix. local/run.py records all local suites and uses a temporary Bash 4.4 build at /tmp/rfig-brew-upgrade-bash44/bash-4.4/bash. Runtime and test source identities are in tested-source.sha256. Each matrix has per-suite logs and summary.json. No unresolved new failures were discarded.

## Delivery state

Changes remain in the local worktree alongside the previously developed Homebrew upgrade changes. No commit, push, release, package replacement, or user Shell/Pnpm configuration change was made for this fix. The installed Homebrew version does not contain the patch yet. Full tests use isolated HOME and fixture history; cleanup.json records process/container cleanup.
