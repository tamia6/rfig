# Full-reference completion regression

## Cause and change

`ocx --help` exposes a short list of common tasks. The remaining commands are
documented in the explicitly advertised `ocx help --all` reference. The old
parser extracted eight invocation rows from the short page and ignored its
multicolumn Explore section. Invoking the same executable as `opencodex` also
missed rows written with its `ocx` alias.

The shared analyzer now discovers the invocation from Usage, and follows an
advertised `<invocation> help --all` command in the existing sandbox, serially
and with the existing time/output limits. It combines the reference with the
short page and deduplicates commands. It does not execute candidate commands or
analyze help during typing. Failure or timeout of the advertised reference
preserves the previous cache instead of replacing it with a partial list.

This applies to all CLI names using the supported help convention. There are
no `ocx` or `opencodex` branches in production code. Arbitrary JSON capability
schemas and arbitrary multicolumn prose are not interpreted as commands.

The regression fixture deliberately uses two names (`tiny`, `rfigreference`)
with a different advertised invocation, and covers sandboxed analysis, cached
commands, visible menus, selection, cancellation, and failed reference refresh
for zsh, Bash and Fish. A failing Rust alias test and a failing reference-cache
test were observed before their fixes.

## Regression infrastructure

The process-table parser added previously split into at most three fields but
required four, yielding an empty table. Its selection loop also unpacked the
wrong number of fields. Both are corrected and a regression asserts that the
current process appears. StrictShell now registers cleanup during setup, checks
liveness using the portable process table and obtains cwd through tmux rather
than Linux-only `/proc`. This makes the full macOS strict suite meaningful.

`AGENTS.md` now requires a complete local and Docker regression after each
feature or fix, including all three shells and process/container cleanup.

## Verification

Final source hashes are in [tested-source.sha256](tested-source.sha256).
No command-name exceptions, new dependencies, unsafe probes or bulk local
command execution were introduced.

Real installed `ocx` and `opencodex` were analyzed explicitly with the development
binary. Their caches each contain all 68 distinct top-level invocations found
in `ocx help --all`, with no missing reference commands. Isolated zsh sessions
verified filtering `pro`, displaying `provider` and accepting it for both names.
The full help was only read; provider/start/stop/configuration commands were not
executed. The two local caches were refreshed; installed binaries were not replaced.

Local macOS regression: **25 of 26 suites passed**, including all 15 Rust unit
tests, installation/setup, safe enrichment, completion sources, help reference,
history, ranking, array handling, large candidate sets, Bash/Fish Emacs and Vi
interaction, prompt geometry, and strict interaction. The remaining suite is
the already recorded zsh slow-provider failure. See [local logs](local/).

Docker uses Debian Bookworm ARM64 with Bash 5.2, separately compiled Bash 4.4,
Fish 3.6 and zsh 5.9. `sh tests/docker/run.sh` runs the complete matrix serially
as an unprivileged user with isolated HOME, no test-stage network, 2 CPU, 2 GiB
memory and a 256-process limit. **46 of 47 Docker suites passed**; only the existing zsh slow-provider case failed. Detailed results are in [docker logs](docker/).
The container was automatically removed after the nonzero matrix exit. A final
process-table check found no test rfig editors or temporary Bash shells remaining.

Reproduction commands:

```sh
cargo test --locked
cargo build --locked
python3 tests/test_process_cleanup.py
RFIG_TEST_SHELL=zsh python3 tests/help_reference.py
RFIG_TEST_SHELL=fish python3 tests/help_reference.py
RFIG_TEST_SHELL=bash RFIG_TEST_SHELL_EXE=/path/to/bash-4.4-or-newer python3 tests/help_reference.py
BUILD_BASH44=1 RFIG_RESULTS_DIR=target/docker-results/full-final-20261022 sh tests/docker/run.sh
```

Full local run logs record the entire existing suite set, including each
shell's strict and prompt-geometry suites. macOS and Docker environment details
are saved alongside this report.

## Remaining limits

The zsh slow-provider test still fails because native ZLE completion invokes a
deliberately sleeping provider synchronously. This matches the earlier baseline;
it is not reported as passing. A help/reference command that exceeds the existing
500 ms probe limit can fail to refresh; it retains its previous cache and can be
retried with `rfig analyze <command>`. Tests do not validate Windows Terminal/WSL.
