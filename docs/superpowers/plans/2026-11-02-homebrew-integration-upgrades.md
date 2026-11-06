# Homebrew integration upgrades

Goal: use one Homebrew executable locally and keep zsh, Bash and Fish adapters synchronized with future Homebrew upgrades.

Architecture: setup recognizes the executable in an active Homebrew keg by comparing the stable opt executable with the running executable. Link each configured adapter to opt/rfig/share/rfig/rfig.<shell>. Other installs keep embedded adapter files. Replace links/files atomically so switching installation methods cannot overwrite package-managed scripts. Shell startup performs no scans or extra commands; existing sessions must restart.

## Implementation and validation

- [x] Add a real-binary fixture with Cellar, an opt symlink, paths with spaces, and all three adapters. Confirm the old setup fails the upgrade assertion.
- [x] Implement stable opt adapter discovery and atomic link/file replacement in setup. Preserve startup configuration and user history, and preserve package files on a switch back to a standalone binary.
- [x] Verify repeated setup, opt switching without rerunning setup, actual shell sourcing, missing packaged adapters, and standalone migration.
- [x] Include the new fixture in the sequential Docker regression matrix. Run the full local matrix and Docker matrix, preserving logs and comparing known zsh slow-completion failure against baseline.
- [x] Update English and Chinese README, website, and Tap caveats with one-time setup and restart requirements.
- [x] Back up and remove the duplicate ~/.local/bin/rfig, use Homebrew as the sole local install, and configure its adapter links. Verify active path, all three shell integrations, history behavior and test process cleanup.

Constraints: no new dependencies, no background scan at shell startup, no automatic commits or publication. Keep failures in testing records, never claim failed or skipped tests passed.
