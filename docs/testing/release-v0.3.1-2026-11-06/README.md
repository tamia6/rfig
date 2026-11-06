# v0.3.1 release and installed Homebrew verification — 2026-11-06

## Publication

- Release commit: `25f25de60d0378e08e3699d59b4b89e6fcc25147`; annotated tag `v0.3.1`.
- [Release workflow](https://github.com/tamia6/rfig/actions/runs/42190206043): macOS universal, Linux x86_64, Linux aarch64 and publishing jobs all succeeded.
- [GitHub release](https://github.com/tamia6/rfig/releases/tag/v0.3.1): three archives published; internal SHA256SUMS verified. Architecture inspection confirms both macOS slices and the two Linux architectures. See release-assets.json and architectures.json.
- Tap commit `813cea3`: source URL uses v0.3.1; SHA256 `8f6f88984f02a9aebc0cb1b3b7cc1df4bd3163b8ed2b9d14730488bcdf5db1cf`. Downloaded tag archive runtime files match this checkout.
- Website commit `ec758ac`: English and Chinese pages deployed; [Pages workflow](https://github.com/tamia6/rfig/actions/runs/42191152184) succeeded. Both live pages contain v0.3.1.

## Installed package

Executed `brew upgrade tamia6/tap/rfig` with `HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_MAKE_JOBS=2 CARGO_BUILD_JOBS=2`, after fast-forwarding the installed Tap. Homebrew upgraded 0.3.0 to 0.3.1. `brew test tamia6/tap/rfig` passed.

The active `/opt/homebrew/bin/rfig` resolves to `/opt/homebrew/Cellar/rfig/0.3.1/bin/rfig`. No duplicate executable exists at `~/.local/bin/rfig`, `~/.cargo/bin/rfig` or `~/bin/rfig`. The configured zsh adapter remains a stable link through `/opt/homebrew/opt/rfig/share/rfig/rfig.zsh`. All three packaged adapter contents match the released source; hashes are in install.json.

No setup or broad background scan was started during the package upgrade. Existing stable adapter links follow the new opt target; existing interactive sessions need to be reopened.

## Actual cf analysis and menu interaction

Ran the installed `/opt/homebrew/bin/rfig analyze cf` against actual HOME, holding the existing `enrich.lock` to avoid another enrichment worker probing concurrently. Probes ran serially under the existing sandbox and time limits. The command exited 0 after about 999 seconds, reporting **65,437 candidates**. The resulting tree contains **3,746 path cache files**. This run logged no unavailable-help branches; normalized repeated-parent responses stopped those branches. Full output is in cf-analyze.log.

Using only this newly generated root/workers cache, a private tmux session for each shell tested **27 workers contexts** at **58 columns**, displaying and selecting an item from every context: **81 context selections total**. Each shell also navigated `cf workers` → `versions` → `list` → `--page`, then cancelled and verified cleanup. These isolated tests used the installed Homebrew binary and packaged adapters; a cf stub rejected execution so browsing could not perform a resource operation. Existing cf-ui/verify_cf.py from the nested-help validation was reused.

All passed: zsh 5.9, Bash 4.4 and Fish 4.9.3. Logs, screen captures and per-shell summaries are in zsh/, bash/ and fish/.

Separately, loaded actual `~/.zshrc` in a new private zsh session with actual HOME/cache. Verified the resolved binary and function source are the installed 0.3.1 package, navigated the same chain using Down/Right, and cancelled cleanly. Prompt was simplified only for this test; history/usage writers were disabled in that session. No cf resource command was executed. Evidence is in actual-user-zsh/.

## Regression scope and cleanup

The runtime source is the same source covered by [the full nested-help regression](../nested-help-2026-11-06/README.md): Rust 20/20, local 36/37 suites and Docker 60/61 suites. The sole remaining failed suite is the already established zsh slow synchronous native-completion input-latency case. It is not represented as passed here. Release-version changes were followed by fresh Rust unit tests, release build and archive-installer checks before tagging; release CI also passed.

Every private tmux/test shell exited through the process cleanup helper. Final process audit found no test-owned rfig, shell or tmux process; no rfig test containers remained. One unrelated, four-week-old stopped container was preserved. See cleanup.json.
