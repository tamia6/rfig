# Issue #6: preserve native path insertion

## Reproduction and cause

In an isolated HOME with only stock zsh completion and rfig loaded, accepting
`cd /` produced `cd Applications ` on macOS. `ls /`, absolute directories,
and `./` paths also lost their prefixes. The new `zsh_path_completion.py`
failed on this exact assertion before the fix.

`compadd -A` returns the quoted match body, not all of its insertion fields.
The capture layer discarded hidden/ignored prefixes and suffixes, while the
selection widget replaced the entire original word. Directory slashes normally
added by native completion were also missing.

## Changes

The shared zsh capture layer uses zsh's option parser to retain compadd insertion
fields, keeping labels separate from inserted words. File candidates retain
native directory slashes. Option values using `--name=path` no longer pass
through the option-name-only filter. No command-name branches were added.

Cross-shell verification also exposed Fish's `~/` candidate being inserted as
`\~/`, preventing home expansion. The Bash/Fish editor now retains a leading
`~/` only when the user actually typed an unquoted, unescaped `~/`. Literal and
quoted tilde candidates still get escaped. Other shell metacharacters remain
quoted; candidate text is never evaluated.

Issue #5 (Enter accepts history suggestions) was not changed. No dependency,
installation, README, website or user configuration changes were needed.

## Verification

Tests cover root paths for both cd and ls, absolute and relative directories,
home paths, escaped spaces, nested selection, files, option-value paths, and
native ignored/hidden insertion fields. The strict suite selects directories
and verifies the shell's actual working directory for zsh, Bash and Fish.
The Bash test registers a directory completion fixture so it also works on
hosts without a system bash-completion installation.

Final logs and source checksums are stored alongside this report. Initial
preflight failures included a stale release binary used by the installer test
and a missing local Bash 4.4 executable; the final run explicitly selects the
current debug binary and a separately built Bash 4.4 in /tmp. Test-stage HOME
and tmux sessions are isolated; the user's actual commands are not scanned.

## Final results

- macOS: **26/27 suites passed** with Bash 4.4, Fish 4.9.3 and zsh 5.9.
- Docker Debian Bookworm: **47/48 suites passed** with Bash 5.2, separately built
  Bash 4.4, Fish 3.6 and zsh 5.9. The matrix runs sequentially without network
  access, as an unprivileged user, limited to 2 CPUs, 2 GiB and 256 processes.
- All 16 Rust tests passed. All new path tests passed, including directory
  selection and execution in every shell/runtime above.
- Both remaining failing suites contain only the existing
  `test_slow_provider_input_latency_and_cancel` failure in zsh. This matches the
  recorded v0.2.2 baseline; it was not fixed or counted as passing. The existing
  zsh Ctrl-D test remains explicitly skipped because native ZLE owns that key.
- Shell syntax, Rust formatting, diff whitespace and final source checksums
  passed. The Docker container was removed. Test-owned editor/Shell processes
  were checked after teardown.

Reproduction:

```sh
python3 docs/testing/issue-6-2026-10-29/local/run.py
BUILD_BASH44=1 RFIG_RESULTS_DIR=target/docker-results/issue6-20261029 sh tests/docker/run.sh
shasum -a 256 -c docs/testing/issue-6-2026-10-29/local/tested-source.sha256
```

The local driver records the exact temporary Bash executable used; recreate it
or adjust its path on a different host. [Local logs](local/) and [Docker logs](docker/)
contain suite summaries and retained terminal screens. No commit, push, release,
local upgrade or GitHub issue status change was performed.
