# Inline gray completion and Command keys — 2026-11-08

Branch: `ux-subtle-tab-accept`

## Interaction

- A single completion or directory-scoped history result renders as an ANSI
  gray suffix after the cursor. The candidate dropdown was removed.
- A compact `2/7` indicator shows the selected position in the ranked set.
- Command+arrow and Command+1…5 change the recommendation on configured
  macOS terminals.
- Option/Alt shortcuts remain available as a fallback.
- Plain arrows keep native history and cursor behavior.
- Tab inserts the selected recommendation; Enter executes only actual input.

`rfig terminal-keys` was added for macOS Terminal.app. It exports and backs up
the current preference file, writes command-key mappings to the active startup
profile, validates the resulting plist, and restores the backup on failure.

## Verification

- `cargo test`: 22 tests, including suffix-only inline preview and unique
  terminal command sequences.
- Real raw `ESC[1;9B` Super+Down sequences passed in zsh, Bash, and Fish.
- `tests/zsh_integration.py`, `tests/zsh_usage.py`,
  `tests/zsh_path_completion.py`, and history-menu tests passed.
- Bash and Fish interactive, strict, prompt-geometry, nested-help, and
  completion-output suites passed.
- `tests/self_completion.py` passed with the new `terminal-keys` command.
- Both README/site demo recordings were regenerated.
- `rfig terminal-keys` was applied and re-applied successfully to the local
  macOS Terminal.app Basic profile.
- The final Docker matrix passed 60/61 tasks. The only failure is the unchanged
  `zsh59-strict` slow-provider baseline described below.

## Baseline failure

`zsh59-strict / test_slow_provider_input_latency_and_cancel` still fails on
unchanged `main` because zsh runs the synthetic slow provider synchronously.
The same failure was reproduced in a separate checkout before this interaction
work. Evidence is in `zsh59-strict.log`.

## Evidence

- `summary.json`: final Docker matrix.
- `zsh_integration.log`: inline gray selection, Tab acceptance, and theme.
- `bash-system-strict.log`, `fish-strict.log`: cross-shell interactive suites.
- `fish-prompt-geometry.log`: narrow prompt and execution cleanup.

macOS Terminal.app Command-key behavior was validated by forwarding the exact
mapped Super sequences into all three shell adapters. The GUI app itself is
blocked from Computer Use, so visual Terminal.app automation is not claimed.
