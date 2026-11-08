# Candidate menu counter — 2026-11-09

The inline suffix now contains only completion text. The selected position and total appear on a gray header line in the candidate menu, above the existing colored numbered rows. This applies to zsh and the shared Bash/Fish editor. Option-arrow bindings are unchanged.

A real tmux assertion checks that the prompt's inline completion line contains no numeric position/total. PTY checks wait for a completed redraw because ZLE need not resend an unchanged counter. Ranking screen checks now find the inline label and menu counter separately.

Validation uses `cargo test --locked`, release/debug builds, the isolated-HOME local Python suites, and the full offline, non-root Docker matrix. Initial zsh PTY/ranking failures were outdated assertions requiring the counter on the input line; the final rerun logs supersede those assertions. Full Docker acceptance evidence is in `docker-final/`, not the initial `docker/` run.

The source adapter and release binary were installed at `~/.config/rfig/rfig.zsh` and `~/.local/bin/rfig`. Previous adapter backup: `~/.config/rfig/rfig.zsh.before-menu-count-20261109`. Existing Terminal.app Option-arrow mappings were preserved. GUI Terminal.app was not directly observed; screen checks use tmux/PTY.

English and Chinese README and local website source reflect the new count placement. No website publication, commit, or push was performed.

Local final results (`local/final-summary.json`): 33/34 suites passed after the four explicit test rechecks. The only remaining failure is the pre-existing zsh slow-provider latency baseline. Initial local summary and logs are preserved, with separate final logs for the corrected PTY, rank-position, and async-candidate readiness tests. Rust unit tests: 22/22 passed.

Final Docker matrix: 60/61 suites passed; `zsh59-strict` fails only the known synchronous slow-provider latency case. All counter-placement, selection, prompt geometry, installation, and other shell tests passed. The test container and temporary image were removed.
