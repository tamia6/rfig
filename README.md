# rfig — Rust Terminal Autocomplete for zsh, Bash, and Fish

**English** · [简体中文](README.zh-CN.md) · [Website](https://tamia6.github.io/rfig/)

**An open-source terminal autocomplete tool with command menus and history-based suggestions.**

Built in Rust, rfig brings IDE-style command-line autocomplete to zsh, Bash, and Fish on macOS and Linux. Candidates appear as you type; navigate with the arrow keys, insert one with the right arrow, and continue completing the next argument. Your shell executes the command.

rfig is an independent, Fig-inspired project and is not affiliated with Fig. It uses completion definitions and scripts available in your environment, with sandboxed help analysis as a conditional fallback; it does not promise completion for every command.

Built-in **history-based autosuggestions** appear as inline gray text and replace the history suggestion functionality of plugins such as `zsh-autosuggestions`. **No separate autosuggestion plugin is required.** Menus use only what you actually type; autosuggestions match history from your current working directory. Both can appear together.

![rfig demonstrating command and path completion with history-based autosuggestions in zsh](assets/rfig-demo.gif)

The demo shows command and path completion, then a menu and an autosuggestion appearing together after typing `gi`. `Ctrl+E` accepts the suggestion; `Enter` executes the full suggestion.

[Installation](#installation) · [Usage](#usage) · [Shell support](#shell-support) · [Completion sources](#completion-sources-and-background-enrichment) · [Testing](#testing)

> rfig v0.2.0 supports macOS and Linux with zsh, Bash 4.4+, and Fish 3.6+. Install with Homebrew, a prebuilt binary, or the source code.

**At a glance:** terminal autocomplete and nested CLI completion · inline, directory-scoped command-history suggestions · local usage-based ranking · no AI service or cloud account required.

## Features

| Feature | Behavior |
| --- | --- |
| Menus as you type | Updates after typing or deleting, without pressing `Tab` or an artificial debounce delay |
| Dynamic, nested completion | Queries available definitions for subcommands, branches, paths, and more; depth and content depend on the source |
| History-based autosuggestions | Matches the current directory and typed prefix; appears alongside the menu and replaces a separate history suggestion plugin |
| Usage-based ranking | Selections and executed commands affect ranking; up to three candidates with usage records are marked `① ② ③` |
| Category icons and theme colors | Category icons, an arrow pointer, and selected text use the terminal's ANSI palette |
| Multiple completion sources | Native definitions, installed scripts, command-provided generators, the Cobra protocol, and cached help data |
| Background enrichment after setup | Quickly scans `$PATH`, then analyzes eligible user commands sequentially on macOS/Linux when a sandbox is available |
| Shell coverage | zsh ✅, Bash 4.4+ ✅, Fish 3.6+ ✅; other shells are not yet supported |

## Installation

### From source

Requires Rust and Cargo. Run one of the following in the project directory:

```sh
sh install.sh                 # Choose integration based on $SHELL
# Or explicitly select a shell
sh install.sh --shell zsh
sh install.sh --shell bash
sh install.sh --shell fish
```

The installer builds rfig into `~/.local/bin/rfig`, then runs `rfig setup`. It does not change your default shell. macOS ships with Bash 3.2, which is not supported; install Bash 4.4+ before enabling Bash integration.

### Homebrew Tap

```sh
brew install tamia6/tap/rfig
rfig setup
```

### Prebuilt binaries (no Rust required)

Choose a package for your operating system and CPU from [GitHub Releases](https://github.com/tamia6/rfig/releases). Package targets are `macos-universal`, `linux-x86_64`, and `linux-aarch64`; check the release for available assets. Extract it, enter the directory containing `install.sh`, and run:

```sh
sh install.sh
```

You can download and extract the package anywhere. Each archive includes the binary and installer. The script verifies the files, moves the binary to `~/.local/bin/rfig`, and runs setup. Source and binary installations use the same script. New packages include a `TARGET` manifest; mismatched packages are rejected before the binary is moved. Linux binaries require glibc 2.36+. Options and package contents depend on the downloaded version.

### Enable multiple shells

Configure each shell separately:

```sh
rfig setup --shell zsh
rfig setup --shell bash
rfig setup --shell fish
```

Open a new session of the configured shell afterward. Setup can be repeated: it writes the integration script, adds `~/.local/bin` when needed, and avoids duplicate source lines. Background enrichment jobs for the same installation run sequentially.

| Shell | Configuration location |
| --- | --- |
| zsh | `$ZDOTDIR/.zshrc`, or `~/.zshrc` by default |
| Bash | `~/.bashrc`; adds a source line to the login configuration if it does not mention `.bashrc` |
| Fish | `$XDG_CONFIG_HOME/fish/conf.d/rfig.fish`, or `~/.config/fish/conf.d/rfig.fish` by default |

If source and Homebrew installations coexist, use `command -v rfig` to check which one is active. To configure the Homebrew version explicitly, run `"$(brew --prefix rfig)/bin/rfig" setup`.

## Usage

Start typing to see candidates. Available completion definitions determine which subcommands and dynamic arguments appear; no menu is shown without a usable source. For example, a Git completion definition can query branches in the current repository.

| Key | Behavior |
| --- | --- |
| `↑` / `↓` | Select a candidate when a menu is visible; otherwise browse history |
| `→` | At the end of the line, insert the selected menu item; without a menu, accept the autosuggestion; within the line, move right |
| `Ctrl+E` | At the end of the line, accept the autosuggestion without executing it; otherwise move to the end |
| `Enter` | Execute the full autosuggestion when present, or the typed command otherwise; does not accept the highlighted menu item |
| `Tab` | Use the shell's native completion without executing the command |
| `Ctrl+C` | Cancel the current input |

The menu displays up to five rows; use the arrow keys to browse the rest. A `-` prefix shows only single-hyphen options; `--` shows only double-hyphen options. This rule applies to all commands.

### History and usage-based ranking

- **Autosuggestions:** rfig maintains its own history and finds recent matches for the current directory and typed prefix. Typing `gi` may suggest `git checkout master`, while the menu still uses only `gi`.
- **Fresh installations:** history starts empty. Old shell history without working-directory information is not imported automatically.
- **Ranking:** scores are accumulated per command context. Selecting a candidate adds 3; recording an execution adds 1. With equal scores, non-option candidates come first. Up to three candidates with usage records receive circled numbers; the rest keep their category icons.
- **Local storage:** history and ranking records stay on your computer. Bash/Fish omit commands beginning with a space; zsh follows that rule when `HIST_IGNORE_SPACE` is enabled.

When using rfig's autosuggestions, remove the `zsh-autosuggestions` source line from `.zshrc` or your plugin manager configuration, then open a new session. Fish integration disables Fish's native autosuggestions and uses rfig's directory-scoped history instead.

### Icons and colors

| Category | Icon | ANSI color |
| --- | --- | --- |
| Command | `⌘` | Cyan |
| Subcommand | `↳` | Magenta |
| Argument | `●` | Green |
| Option that takes a value | `◇` | Blue |
| Boolean flag | `⚑` | Yellow |

Colors follow the terminal's ANSI palette; rfig does not parse individual terminal theme files. Circled numbers keep the category color, and the selected item uses `→` and cyan text. Moving between candidates adds brief shifts, glyph changes, and bold text—a character animation rather than actual font scaling.

Categories use available metadata. Without option analysis, `--` entries are provisionally treated as options and single `-` entries as flags, so classification may be approximate.

## Shell support

| Capability | zsh | Bash 4.4+ | Fish 3.6+ |
| --- | --- | --- | --- |
| Menus, nested candidates, autosuggestions separate from typed input | ✅ | ✅ | ✅ |
| Directory-scoped history, usage ranking, category colors, circled numbers | ✅ | ✅ | ✅ |
| Setup integration and completion for rfig itself | ✅ | ✅ | ✅ |
| Multiline paste without automatic execution | ✅ | ✅ Returns to native editor | ✅ Returns to native editor |

zsh uses ZLE; Bash/Fish share a Rust editing layer. `cd`, functions, and variable changes still run in the current shell. Bash/Fish support default Emacs editing and Vi insert mode. `Esc` returns to the native editor; `Ctrl+R` uses native history search. Stale completion results are discarded.

**Linux requirements:** x86_64 and ARM64 are supported, with the shell versions listed above. Help analysis uses built-in Landlock + seccomp and needs no additional sandbox tool. It requires Landlock ABI 3+ (typically Linux 6.2+) and permission to use the relevant system calls. Older kernels or restricted containers refuse help probing; menus, history, and existing completion definitions remain available. Check the kernel with `uname -r` and actual probing with `rfig analyze <command>`. A distribution name alone does not determine kernel capabilities.

macOS uses the system `sandbox-exec`. Linux binaries target glibc 2.36; older userspaces may try building from source. Windows and other shells are not yet supported.

## Completion sources and background enrichment

### While typing: candidates from your environment

rfig does not maintain a fixed completion database for every command. Each adapter uses available sources:

| Entry point | Lookup behavior |
| --- | --- |
| zsh | Registered definitions first; otherwise `$fpath` scripts, generated zsh scripts, external sources, and cached help data |
| Bash/Fish | Current-shell definitions and autoloaded scripts, generated scripts for that shell, external sources, and cached help data |
| External sources | Recognized Cobra `__complete` protocols and locally installed Fish/Bash scripts |
| Help cache | Extracted top-level subcommands and options when no other source is available; does not replace arbitrary-depth dynamic completion |

Definitions and loading behavior differ between shells, so candidates may differ. Background enrichment does not eliminate queries while typing: dynamic completion scripts still run when needed. Use trusted completion scripts.

### After setup: quick scan, then sequential enrichment

1. **Scan:** enumerate executables and symbolic links in the current `$PATH`, and index zsh definitions and common zsh/Fish/Bash completion directories. This is not a whole-disk scan; programs outside `$PATH` may not be discovered.
2. **Select:** skip commands with existing definitions. Automatically probe only permitted, common user installation directories. System commands and other sources may be cataloged without being executed automatically.
3. **Enrich:** on macOS/Linux, read `--help`, then `-h` if needed, one command at a time in an available sandbox. Identify advertised generators, generate and syntax-check scripts for installed shells, recognize Cobra protocols, and cache subcommands and option categories. Recognition and syntax checks do not guarantee semantic correctness for every candidate.
4. **Constrain:** probing denies file writes and network access, with a 500 ms timeout per process invocation plus CPU and output limits. Linux additionally limits file metadata changes, process-group escape, address space, and process count. Writes to pre-opened output pipes/files and `/dev/null` are allowed. Script syntax checks also use the sandbox and timeout. Complex, resource-intensive, or network-dependent commands may yield no results. See [Linux implementation and boundaries (Chinese)](docs/linux.md).
5. **Refresh:** rerunning setup reanalyzes caches older than the command executable or rfig. Some commands may have no menu until enrichment finishes or a usable definition is available.

```sh
rfig setup                         # Rescan after PATH or installed commands change
cat ~/.config/rfig/enrich.status    # View analyzed / skipped / failed counts
rfig analyze <command>             # Refresh one command's cache; requires a sandbox
rfig --help                        # List public commands and usage
```

In `enrich.status`, `done` means processing has finished, not that every command succeeded. `unavailable` means the environment lacks the required isolation support. `rfig setup --help` only prints help; it does not install or scan.

## Local data and uninstalling

| Path relative to `~/.config/rfig/` | Purpose |
| --- | --- |
| `commands.txt` / `supported.txt` | Discovered command names / commands with discovered definitions |
| `rfig.zsh` / `rfig.bash` / `rfig.fish` | Integration scripts written from the binary during setup |
| `generated/*.{zsh,bash,fish}` / `protocol/*` | Generated, syntax-checked scripts / recognized protocols |
| `options/*.tsv` / `fallback/*.tsv` | Option categories / candidates extracted from help |
| `enrich.status` / `enrich.lock` | Background progress / lock preventing duplicate jobs |
| `usage.log` | Candidate selection and command execution records for ranking |
| `history.tsv` | Command history with working directories, permissions `0600` |

Delete `usage.log` to reset ranking, or `history.tsv` to clear autosuggestion history, then restart the shell. History contains command text; manage it according to your needs.

To uninstall, remove rfig's source line from each configured shell. For Fish, remove `conf.d/rfig.fish`. Delete `~/.local/bin/rfig` for source/binary installations, or run `brew uninstall rfig` for Homebrew. Remove `~/.config/rfig/` when its data is no longer needed, then restart the shell.

## FAQ

### Is rfig a Fig alternative?

rfig is an independent, open-source terminal autocomplete project inspired by Fig. It is not affiliated with Fig and does not claim compatibility with every Fig feature or specification.

### Does rfig complete every command?

No. Candidate coverage depends on completion definitions, scripts, generators, and cached help available on your system. Some commands may have no candidates or only basic help-derived candidates.

### Does rfig use AI?

No. Completion, history suggestions, and usage ranking run locally; no AI service or cloud account is required.

## Testing

Tests send real keystrokes through tmux/PTY and check the screen and arguments received by commands. Coverage includes autosuggestions, menus, ranking, directory-scoped history, special characters, multiline paste, and process cleanup.

```sh
cargo test
cargo build
sh tests/docker/run.sh
RFIG_TEST_IMAGE=rust:1.91-slim BUILD_BASH44=0 sh tests/docker/run.sh
```

Docker tests use an isolated HOME, an unprivileged user, resource limits, and no network during testing. Results are saved in `target/docker-results/`. See the [test instructions](tests/docker/README.md).
