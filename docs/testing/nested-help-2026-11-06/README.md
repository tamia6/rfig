# Nested help cache and Docker menu validation — completed 2026-11-06

## Implementation

Help analysis walks declared subcommands breadth first, without a fixed depth limit. Every path has an atomic TSV cache; readers validate each parent-child transition and distinguish option values from subcommands. Real CLI help formats with full invocation prefixes, wrapped descriptions and wrapped type annotations are supported. A repeated normalized parent help page stops that branch. Failed help invocations preserve prior cache data. Runtime completion reads cache instead of launching new help scans.

The default Fish cf definition in Debian is for Cloud Foundry, not the installed Cloudflare command. Under cf workers it returns filesystem fallback. A shared rule now uses cached children at known groups when a source returns only existing filesystem paths, and fills empty native results from cache. Nonempty native leaf dynamic arguments remain available. This rule contains no cf-specific branch.

## Final results

- Rust: **20/20 unit tests pass**.
- Local full regression: **36/37 suites pass**, across zsh 5.9, Bash 4.4 and Fish 4.9.3.
- Docker full regression: **60/61 suites pass**, Debian Bookworm with zsh 5.9, Bash 5.2, Bash 4.4 and Fish 3.6. The test stage runs offline as tester, with init, 2 CPUs, 2 GiB memory and 256 PIDs. Final tested runtime and test source files match the host exactly (tested-source.sha256).
- The sole remaining failed suite in both matrices is the existing zsh slow native completion case, test_slow_provider_input_latency_and_cancel; it matches the earlier baseline. The independent-editor SIGTERM case is skipped for zsh because it uses ZLE. No new failed suite is hidden.
- Help-only fixtures exercise two CLI help styles, an 18-subcommand-deep chain to its leaf, option values before subcommands, leaf flags, parent-page repetition, failed-child cache retention, filesystem fallback, and preservation of native dynamic leaf arguments.

## cf workers: actual cache, real terminal interactions

Host help capture produced **3,738 path caches** and 65,323 candidate records. Three branches could not provide help and were reported/skipped; one is cf workers check. Their names are in real-cf/summary.json. This capture was collected during implementation; the final cache reader and fallback policy were subsequently verified against its recorded data. Failed-cache retention is also tested independently with an explicit failing help fixture.

Copied only the root and workers subtree from that actual help-derived cache into the isolated container test HOME. The available workers subtree contains **26 cached contexts**, including 6 groups. The real Cloudflare CLI is not installed/executed in the container; an executable stub logs and rejects any attempt to execute it. Native shell completion definitions, including Debian's unrelated cf.fish, remain enabled.

For each of zsh, Bash 5.2, Bash 4.4 and Fish 3.6:

1. Audit all 26 worker contexts through the final cache reader and compare records with the captured cache.
2. At 58 columns, display all 26 menus (groups normally, leaf options using --), select an actual row and insert it with Right. Cancel between cases.
3. Specifically navigate Down to versions, insert it, navigate Down to list, insert it, then choose --page. Confirm the full input is cf workers versions list --page.
4. Check cancellation clears the menu and the cf stub was never executed.

**All four configurations pass: 104 displayed-and-selected contexts total.** Per-shell summaries and terminal text captures are in docker/cf-ui-*/. The driver and cache snapshot are in cf-ui/; cf-ui/summary.json records the four successful runs. These are terminal text captures, not GUI/font screenshots. cf workers check had no successfully captured help and is outside this 26-context proof; its parameter hints are not guaranteed by this validation.

## Cleanup and earlier failures

During validation, Fish's asynchronous completion-cache writer exposed a test cleanup race. The helper previously killed tmux before recording pane descendants, allowing detached children to be reparented and missed. It now captures descendants before killing tmux. A deterministic detached-writer test failed before this change and passes afterward. The full matrices were rerun after the fix.

Earlier provisioning failed downloading GNU Bash. The runner can reuse target/test-deps/bash-4.4.tar.gz, validating pinned SHA256 d86b3392c1202e8ff5a423b302e6284db7f8f435ea9f39b5b1b20fd3ac36dfcb before building the real Bash 4.4. Earlier logs remain in ../nested-help-2026-11-05/preflight/.

The cf-specific UI fixture initially passed a standalone -- through tmux's option parser; this was a driver argument issue. It now passes an explicit argument separator. The final runs all pass; the preceding logs are retained in preflight.

After collecting all evidence, audited container test processes, removed rfig-verify-164, and checked host test processes and Docker inventory. **No active test process or test container remains.** cleanup.json and docker/container-process-audit.json record this.

## Reproduction and delivery state

From repository root: cargo test --locked -j 2; run local/run.py at this report's path; or RFIG_RESULTS_DIR=<directory> sh tests/docker/run.sh. The local driver uses the temporary Bash path recorded in local/run.py. To repeat the recorded-cache UI proof, copy cf-ui/ into /work of a provisioned test container and run verify_cf.py with RFIG_TEST_SHELL, RFIG_TEST_SHELL_EXE and RFIG_TEST_ARTIFACTS, sequentially for the four configurations.

No commit, push, release or local Homebrew package upgrade was performed. The working tree includes earlier unpublished Homebrew and output-isolation work. These validation results refer to the current source, not the published Homebrew v0.3.0 executable. English/Chinese README and site changes remain local.
