# Narrow-window prompt rendering regression

## Fix

- Bash evaluates the prompt once per `PROMPT_COMMAND` cycle, preserves its
  original source for future prompts, and reuses the expanded text while editing.
  The `promptvars` setting and prompt exit status are preserved.
- Removed rfig's DECSC/DECRC prompt anchor. Readline clears its current physical
  line before `bind -x`; the Rust adapter reconstructs the frozen prompt from a
  live cursor query and physical layout (wrapping, Unicode, SGR/OSC and common
  cursor controls), then measures its actual input position.
- Bash cleanup preserves preceding prompt lines. Rust's shared Bash/Fish editor
  queries the live cursor again after resize instead of only clamping stale
  coordinates.
- zsh uses ZLE's incremental refresh after selection instead of `reset-prompt`.
  Menu and description widths follow `COLUMNS`, preventing padded rows from
  wrapping and pushing stale overlays into scrollback.
- No command-specific branches or candidate changes were added.

## Reproduction and tests

Before the fix, the isolated Bash regression failed because entering the editor
executed the dynamic prompt again (`prompt-calls` changed from `x` to `xx`).
The zsh regression also exposed leftover menu rows after selection/execution.

`tests/prompt_geometry.py` reuses the tmux/PTY harness with isolated HOME and
synthetic commands. It checks:

1. A long, multiline prompt with a dynamically generated time, at 58 columns.
2. Resize with a visible menu through 38, 72 and 58 columns, at 10 rows.
3. Selection and literal argv execution, followed by menu cleanup.
4. Twenty lines of output and repeated editing/execution at the screen bottom;
   editing must not accumulate additional prompt headers.
5. A 95-character argument wrapping across rows during resize.
6. ANSI-positioned right-side prompt content in Bash/zsh, and native
   `fish_right_prompt` in Fish (visibility remains under Fish's control).
7. Bash with `promptvars` disabled.

The PTY integration test now inspects the actual ZLE buffer after selection;
it no longer requires a full prompt to be emitted in one raw output chunk.
The terminal-emulator tests separately check visible rendering and cleanup.

## Docker verification

Linux ARM64, Debian Bookworm; Bash 5.2.15, Fish 3.6.0 and zsh 5.9.
See [environment](environment.txt), [container limits](container.txt) and
[tested source hashes](tested-source.sha256).

Tests ran sequentially as `tester`, without network access, with 2 CPUs,
2 GiB RAM, a 256-process limit and init enabled. Personal shell configuration
was not loaded. The regression is included in `tests/docker/run_matrix.py`.

| Suite | Result |
| --- | --- |
| Rust unit tests | 13 passed |
| Prompt geometry: Bash | 4 passed |
| Prompt geometry: Fish | 3 passed, Bash-only option test skipped |
| Prompt geometry: zsh | 3 passed, Bash-only option test skipped |
| Bash/Fish Emacs and Vi interaction | All four suites passed |
| Bash/Fish strict interaction | All 16 cases passed |
| zsh integration, history, usage | Passed |
| Setup safety | Passed |
| zsh strict interaction | 6 passed, 1 skipped, 1 existing failure |

Logs, terminal captures and the full summary are retained in [logs](logs/).
Replay the focused tests inside the provisioned container:

```sh
RFIG_TEST_SHELL=bash python3 tests/prompt_geometry.py
RFIG_TEST_SHELL=fish python3 tests/prompt_geometry.py
RFIG_TEST_SHELL=zsh python3 tests/prompt_geometry.py
```

## Remaining boundary

The existing zsh slow-provider test still fails: a completion script deliberately
sleeping for five seconds blocks ZLE input. This is separate from prompt
geometry and is not marked as passing. The broader matrix therefore does not
have an all-green result.

Docker/tmux does not validate Windows Terminal/WSL's own reflow behavior or an
actual oh-my-posh installation. The tests exercise equivalent dynamic,
multiline and right-side prompt layouts. No installed binary, personal shell
configuration, release or remote repository was changed.
