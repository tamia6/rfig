# Multi-shell implementation plan

Goal: deliver zsh, Bash (4.4+), Fish (3.6+) integration without changing the user's shell or pushing code.

Architecture: preserve zsh's native ZLE adapter. Bash/Readline and Fish hand a single editing session to a shared Rust editor; only the shell executes accepted text. Rust owns candidate filtering, usage ranking, directory history and inline rendering. Native Tab/search/vi escape return control to the shell. Each Shell's installed completion definitions remain the first source; cached help and external providers are fallbacks. No command-name special cases.

- [x] Add isolated setup and PTY regression tests that fail before implementation.
- [x] Add shared candidate/history engine and asynchronous completion worker (one in flight, stale results discarded).
- [x] Add inline editor with menu, gray suffix, colors, pointer pulse, Unicode editing, cancellation, resize and cleanup.
- [x] Add Bash/Fish adapters; preserve native execution, Tab, search and cursor editing handoff. Document version floors and unsupported modes.
- [x] Update setup, completion CLI, installer, release bundle; embed integration scripts so existing Homebrew formula can install all adapters.
- [x] Run Rust tests, existing zsh suites, Bash/Fish PTY suites and isolated installation tests. Use local shells and Docker Bash without user HOME mounts; no broad host command scanning.
- [x] Update README and local website support matrix based on measured results. Leave changes uncommitted and unpublished.

Acceptance: typed input alone controls menu; Enter accepts ghost or executes typed line; right accepts candidate; Ctrl+E accepts ghost; history stays directory scoped; '-' and '--' filtering applies to all commands; top 3 used candidates numbered; overlay never enters command output. Terminal palette uses ANSI slots. Candidate subprocesses bounded and input stays responsive. Installation idempotent per shell and accepts explicit --shell.

## Verification (2026-10-03)

- `cargo test -j 2`: 12 passed; `cargo build`, `cargo fmt --check`, `git diff --check` passed.
- macOS zsh 5.9: integration, usage ranking, directory history suites passed.
- macOS Fish 4.9.3: interactive suite passed in default and vi insertion modes with multiline prompts.
- Linux Docker (2 CPUs, 2 GiB): Bash 5.2.37 and Fish 4.0.2 interactive suites passed in default and vi insertion modes.
- Isolated setup, source/archive installer, background safety and CLI completion suites passed. Setup migrates previous Homebrew source paths without duplication.
- Verified real Enter/right/Ctrl+E/Tab/cancel input, command execution in the host shell, prefix-only menu context, nested providers, help fallback, usage ranking, dash filtering, Unicode editing, resize and popup cleanup.
- Bash minimum 4.4, Fish minimum 3.6 are API requirements; these minimum versions were not directly exercised. macOS Bash 3.2 is rejected with an upgrade message. Linux background help execution stays disabled without the macOS sandbox.
- No host-wide scan, installation, commit, push, or website publication performed.
