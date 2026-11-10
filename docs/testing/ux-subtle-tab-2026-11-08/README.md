# Quiet menu and Tab acceptance — 2026-11-08

Branch: `ux-subtle-tab-accept`

## Interaction changes

- Candidate rows use muted ANSI gray; the focused row uses `›` and bold text.
- Option/Alt plus an arrow or visible number moves focus.
- Tab inserts the focused candidate. With no candidate menu, Tab falls back to
  the shell's native completion.
- Plain arrows keep native history and cursor behavior.
- Right no longer inserts a candidate; dedicated regressions cover zsh, Bash,
  and Fish.
- Super/Command sequences are accepted when the terminal forwards them.
- The previous pulse animation and circled popularity badges were removed.
- Bash/Fish preserve explicit focus if a completion update arrives before Tab.

## Verification

The following passed:

- `cargo test`: 20 unit tests.
- `python3 tests/zsh_array_slices.py`: 4 tests.
- `python3 tests/zsh_integration.py`.
- `python3 tests/zsh_usage.py`.
- `python3 tests/zsh_path_completion.py`.
- `RFIG_TEST_SHELL=zsh python3 tests/history_menu.py`.
- Bash and Fish `tests/multi_shell_interaction.py`.
- Bash, Fish, and zsh `tests/completion_output.py`,
  `tests/help_reference.py`, and `tests/prompt_geometry.py`.
- Bash and Fish `tests/strict_shell.py`.
- `uv run --with pillow --with pyte python3 scripts/record_showcase.py`.
- `uv run --with pillow --with pyte python3 scripts/record_demo.py`.

The full `sh tests/docker/run.sh` matrix passed every case except the known
`zsh59-strict / test_slow_provider_input_latency_and_cancel` baseline. The first
Docker pass also exposed a Fish cleanup race after execution; clearing every row
below the prompt on execute fixed it, and the second full matrix passed that
case.

## Known baseline failure

`test_slow_provider_input_latency_and_cancel` fails identically on unchanged
`main` in a separate checkout when the slow zsh provider is active:

```text
RFIG> rfigslo
› 1 ⌘ rfigslow
Alt+↑↓ select · Alt+1–5 jump · Tab accept
```

This branch does not change that synchronous zsh provider path. Evidence is in
`zsh59-strict.log`.

## Evidence

- `summary.json`: second Docker matrix result.
- `zsh59-strict.log`: unchanged baseline failure.
- `fish-prompt-geometry.log`: passing Fish cleanup case.
- `zsh_integration.log`, `fish-strict.log`,
  `bash-system-strict.log`: passing cross-shell interaction evidence.
- `assets/rfig-showcase.gif` and `assets/rfig-demo.gif`: regenerated recordings.

Docker does not validate a macOS GUI terminal's Command-key handling. Terminal
may consume Command+arrow itself; Option/Alt is the portable default documented
in the README.
