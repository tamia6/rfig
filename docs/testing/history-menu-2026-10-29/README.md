# History suggestions as explicit menu items

## Approved behavior

At most one recent command matching the actual typed prefix and current directory
appears first, marked `◷`. The rest of the menu uses only the user's input.
There is no inline gray suggestion. Enter executes only the actual buffer;
Right at the end of the line accepts the selected row without executing it.
History replaces the full buffer; ordinary completions replace their current
word. Ctrl+E moves to the end without accepting anything. History does not use
or displace ordinary candidates' `① ② ③` usage markers. Five rows remain visible;
more candidates remain reachable with the arrows. No history row is reserved
when no match is available.

## Implementation and checks

The Rust editor adds history after ordinary completion filtering/ranking, or
immediately while asynchronous completions are pending. Native zsh completion
is kept separate and the wrapper prepends history before rendering. History
selection never quotes the whole saved command as a single argument. Neither
Enter nor Ctrl+E appends suggestions in any shell. Existing storage formats,
directory matching, normal usage scoring, and native Tab completion remain.
Control characters from history files are excluded from menu rows.

`tests/history_menu.py` sends real keystrokes in isolated HOME/tmux sessions for
all three shells. It checks first-row history, no inline text, unchanged normal
prefix completion, popular markers, explicit whole-command insertion,
ordinary selection below history, Enter and Ctrl+E behavior, history without
native definitions, directory scope, cancellation, cd execution, and invalid
history records. The old zsh history script now runs this shared contract.
The new interaction test failed on the previous inline behavior before code
changes. Rust tests cover history precedence and unchanged usage markers.

Existing interaction, ranking and nested tests were updated to navigate past
the history row. The fake Homebrew setup fixture now cleans its background
worker before deleting its HOME, fixing an intermittent fixture cleanup race.
Previously completed issue #6 path fixes remain included and tested.

README and website text were updated in both languages. Both animation
recorders ran in a disposable HOME/repository; the generated showcase was
copied into the local website checkout. No site deployment or local application
upgrade was performed.

## Final regression results

- Local macOS: **29/30 suites passed**, including all 17 Rust tests, source and
  binary installation, repeated setup, safe scanning/enrichment, all completion
  sources, history menus, usage ranking, nested paths/options, large candidate
  sets, zsh native interaction and Bash/Fish Emacs and Vi modes.
- Docker Debian Bookworm: **51/52 suites passed**, covering Bash 5.2, Bash 4.4,
  Fish 3.6 and zsh 5.9. The matrix runs serially as an unprivileged user with no
  test-stage network, 2 CPUs, 2 GiB RAM, a process limit, and automatic cleanup.
- The history-menu tests passed in every shell/runtime. Narrow windows,
  multiline dynamic prompts, scrolling, resize, literal characters,
  cancellation and process cleanup were included. No test-owned editor/Shell
  process or verification container remained after the final matrix.
- The sole failing suite in each environment contains the pre-existing zsh
  `test_slow_provider_input_latency_and_cancel` failure. The native zsh Ctrl-D
  case remains explicitly skipped. These match the previous baseline and are
  not represented as passing.
- The first Docker run exposed a test sequencing race: Ctrl+C was sent while
  Ctrl+E's synchronous native completion callback was still running. The
  fixture now waits for a ZLE widget to confirm completion before checking
  idle cancellation. Both complete matrices were rerun after this test change.
  This does not fix the known slow native-completion behavior. Initial failed
  logs are retained in [preflight/](preflight/).
- Final source checksums, suite summaries and terminal screens are retained in
  [local/](local/) and [docker/](docker/). Rust formatting, shell syntax and diff
  whitespace checks passed. Both demonstration scripts also completed;
  [a rendered capture](history-menu.png) shows the new history row.

Reproduction from the repository root:

```sh
python3 docs/testing/history-menu-2026-10-29/local/run.py
BUILD_BASH44=1 RFIG_RESULTS_DIR=target/docker-results/history-menu-final-20261029 sh tests/docker/run.sh
shasum -a 256 -c docs/testing/history-menu-2026-10-29/local/tested-source.sha256
```

The local driver records the temporary Bash 4.4 executable path used; recreate
it or adjust that path on another host. No code was committed/pushed, no release
was published, and the user's installed rfig was not replaced.
