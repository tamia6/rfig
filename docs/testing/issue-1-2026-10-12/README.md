# Issue #1: macOS completion verification

## zsh fix and regression

The path is `self-insert` → `_rfig_after_edit` → `_rfig_preview` →
`_rfig_capture` → native `_lsof` → `_ports` → `_rfig_compadd`.
Each individual append to the unique `_rfig_hits` array repeated its
deduplication work. The fix collects records locally and appends one batch;
candidate validation, quoting, classification, ordering and deduplication
remain intact. No command-specific exception or safety relaxation was added.

The isolated `tests/zsh_large_completion.py` regression failed before the
fix at 687 ms for 5,000 ports and passed after it at 145 ms for `lsof -i :`
and 128 ms for `telnet localhost `. All 5,000 candidates were retained in
order without duplicates, with argument classification and the inline menu
preserved. The check is included in `tests/docker/run_matrix.py`.

`cargo build`, all 12 Rust unit tests, `tests/zsh_integration.py`,
`tests/zsh_usage.py`, the zsh literal-candidate safety check and
`tests/setup_safety.py` passed. Linux/Docker execution remains unverified.

## Bash/Fish follow-up

Verified on 2026-10-12 with the current `target/debug/rfig`, GNU Bash
4.4.0(1)-release and Fish 4.9.3, using isolated HOME directories and tmux.
No personal shell configuration or default shell was changed.

Bash was built under `/tmp/rfig-bash-verification/bash-4.4` from
`https://mirrors.kernel.org/gnu/bash/bash-4.4.tar.gz`.
Source SHA256: `d86b3392c1202e8ff5a423b302e6284db7f8f435ea9f39b5b1b20fd3ac36dfcb`.
Configure used `--without-bash-malloc --disable-nls`; the old source needed
Clang compatibility flags and `-D_FORTIFY_SOURCE=0` to build on this SDK.
These flags apply only to the temporary test shell, not rfig.

## Existing tests

All passed for both shells:

- `tests/multi_shell_interaction.py`, default Emacs mode and `RFIG_TEST_VI=1`.
  Covers inline menus, nested completion, history suggestions, ranking,
  option filtering, cancellation, Unicode, Tab delegation and resize.
- `tests/strict_shell.py StrictShell.test_literal_candidates
  StrictShell.test_rapid_input_delete_and_wrapping
  StrictShell.test_multiline_bracketed_paste`.
  Covers literal argv (including `$(touch PWN)` without evaluation), rapid
  editing, wrapping and paste requiring Enter before execution.

Run each with `RFIG_TEST_SHELL=bash|fish`; Bash additionally used
`RFIG_TEST_SHELL_EXE=/tmp/rfig-bash-verification/bash-4.4/bash`.

## Focused input checks

The assert-based [verify_ports.py](verify_ports.py) reuses `StrictShell`'s
isolated tmux harness. Its process-liveness check uses `ps` instead of the
existing harness's Linux-only `/proc` implementation. Replay from the repo root:

```sh
RFIG_TEST_SHELL=fish python3 docs/testing/issue-1-2026-10-12/verify_ports.py
RFIG_TEST_SHELL=bash RFIG_TEST_SHELL_EXE=/path/to/bash-4.4-or-newer python3 docs/testing/issue-1-2026-10-12/verify_ports.py
```

| Check | Bash 4.4 | Fish 4.9.3 |
| --- | --- | --- |
| `lsof -i :`, colon observed on screen | 12 ms | 13 ms |
| `nc localhost :`, colon observed on screen | 13 ms | 11 ms |
| Synthetic 5,000-candidate list, colon observed on screen | 10 ms | 15 ms |
| Slow-provider input and cancel observation | 22 ms | 76 ms |

The synthetic list used current-session completion definitions for
`rfigprobe`, with `:service0001` through `:service5000`. `rfig query` returned
all 5,000 in order. The inline menu appeared, and filtering then accepting
the last candidate executed exactly the literal argument `:service5000`.
The actual command checks typed input and cancelled without executing it;
they do not imply that the shells' native definitions supply identical port
candidates. Timing includes tmux/screen observation overhead.

The slow-provider check also verified cancellation removed provider
descendants and did not display stale results. No Bash/Fish production
changes were necessary.

## Limits

Linux/Docker was not verified in this follow-up. Docker was unavailable;
the existing Podman VM stopped immediately after startup. Homebrew's Bash
download failed DNS resolution, so the temporary source build was used.
This is a targeted completion check, not a full platform acceptance run.

During the pre-push concurrent Bash/Fish replay, Fish's slow-provider test
failed once because the provider did not start within its three-second
wait. The colon and 5,000-candidate checks still passed. A subsequent
standalone Fish replay passed all three checks (11 ms for the `lsof` colon,
12 ms for the large-list colon, 27 ms for slow-provider input/cancel).
The cause of the intermittent provider-start failure was not established.
