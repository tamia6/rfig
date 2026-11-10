# Refresh terminal recordings — 2026-11-09

Re-recorded both GIFs from the current zsh adapter in isolated HOME/repositories through tmux. The previous assets incorrectly showed a gray horizontal candidate strip and counters in the inline suffix. The npm scene also accepted a history command before claiming to show dynamic script selection.

The new showcase types a space to expose package.json script candidates, uses Option+Down, selects candidate 2 through the ESC+2 sequence forwarded by the configured Terminal.app Command+2 mapping, inserts build with Tab, and verifies build output after Enter. Snapshot assertions verify candidate availability, selected position, and actual command buffer. This is a tmux recording, not a physical GUI Command-key test.

All recorded input lines were checked to contain no position/total counter. The menu has colored aligned rows and its own counter. Apple Symbols provides the history and category glyphs; key frames were visually inspected after final encoding. The demo has 12 frames / 15.25s; the showcase has 17 frames / 18.35s. Sanitized buffer snapshots are included here.

Commands: `uv run --with pillow --with pyte python scripts/record_demo.py` and `uv run --with pillow --with pyte python scripts/record_showcase.py`. Both passed. Website and README GIF files were compared byte-for-byte after synchronization. Application code was unchanged; the existing complete regression evidence remains under ../menu-count-2026-11-09/.
