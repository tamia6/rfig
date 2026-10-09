"""Record a safe, disposable multi-command rfig showcase GIF.

Run with:
    uv run --with pillow --with pyte scripts/record_showcase.py

The recording uses a temporary HOME, repository, package manifest, and history
file. It never reads or changes the user's real shell state.
"""

from __future__ import annotations

import json
import os
import re
import shlex
import shutil
import subprocess
import tempfile
import time
from pathlib import Path

import pyte
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "assets/rfig-showcase.gif"
EVIDENCE = ROOT / "target/showcase-recording"
EVIDENCE.mkdir(parents=True, exist_ok=True)


def load_font(size: int, symbol: bool = False):
    candidates = (
        (os.environ.get("RFIG_DEMO_SYMBOL_FONT"), "/System/Library/Fonts/Supplemental/Arial Unicode.ttf")
        if symbol
        else (os.environ.get("RFIG_DEMO_FONT"), "/System/Library/Fonts/Menlo.ttc")
    )
    for candidate in candidates:
        if candidate and Path(candidate).exists():
            return ImageFont.truetype(candidate, size)
    fallback = "/System/Library/Fonts/Supplemental/Arial Unicode.ttf" if symbol else "/System/Library/Fonts/SFNSMono.ttf"
    return ImageFont.truetype(fallback, size)


FONT = load_font(20)
SMALL = load_font(14)
SYMBOLS = load_font(20, symbol=True)
BG = "#262633"
FG = "#d5d8ed"
COLORS = dict(
    zip(
        [
            "black", "red", "green", "brown", "blue", "magenta", "cyan", "white",
            "brightblack", "brightred", "brightgreen", "brightbrown", "brightblue",
            "brightmagenta", "brightcyan", "brightwhite",
        ],
        [
            "#262633", "#ed8796", "#a6da95", "#eed49f", "#8aadf4", "#c6a0f6",
            "#8bd5ca", "#d5d8ed", "#9399b2", "#ed8796", "#a6da95", "#eed49f",
            "#8aadf4", "#c6a0f6", "#8bd5ca", "#ffffff",
        ],
    )
)


def ansi_color(value: str) -> str:
    if value == "default":
        return FG
    return COLORS.get(value, "#" + value if len(value) == 6 else FG)


def render(ansi: str, cursor: tuple[int, int], title: str) -> Image.Image:
    screen = pyte.Screen(80, 12)
    pyte.Stream(screen).feed(ansi.removesuffix("\n").replace("\n", "\r\n"))
    image = Image.new("RGB", (1120, 430), "#101015")
    draw = ImageDraw.Draw(image)
    draw.rounded_rectangle((18, 16, 1102, 414), radius=24, fill=BG, outline="#414152", width=2)
    draw.rounded_rectangle((20, 18, 1100, 66), radius=21, fill="#30303d")
    draw.rectangle((20, 42, 1100, 66), fill="#30303d")
    for x, fill in ((50, "#ff5f57"), (76, "#ffbd2e"), (102, "#28c840")):
        draw.ellipse((x - 7, 31, x + 7, 45), fill=fill)
    draw.text((140, 29), title, font=SMALL, fill="#b9bfd3")
    draw.text((942, 29), "rfig showcase", font=SMALL, fill="#9399b2")

    for row in range(12):
        for col in range(80):
            cell = screen.buffer[row][col]
            if not cell.data:
                continue
            draw.text(
                (38 + col * 13, 82 + row * 27),
                cell.data,
                font=SYMBOLS if cell.data in "⌘↳●◇⚑→➜①②③" else FONT,
                fill=ansi_color(cell.fg),
                stroke_width=0.25 if cell.bold else 0,
            )
    x, y = cursor
    draw.line((38 + x * 13, 85 + y * 27, 38 + x * 13, 106 + y * 27), fill=FG, width=2)
    return image


with tempfile.TemporaryDirectory(prefix="rfig-showcase-") as temp:
    home = Path(temp)
    work = home / "project"
    work.mkdir()
    (work / "src").mkdir()
    (work / "package.json").write_text(
        json.dumps(
            {
                "name": "rfig-demo",
                "scripts": {
                    "build": "printf 'build complete\\n'",
                    "dev": "printf 'dev server ready\\n'",
                    "test": "printf '3 tests passed\\n'",
                },
            }
        )
    )

    subprocess.run(["git", "init", "-q", "-b", "main", str(work)], check=True)
    subprocess.run(["git", "-C", str(work), "config", "user.email", "demo@example.com"])
    subprocess.run(["git", "-C", str(work), "config", "user.name", "rfig demo"])
    (work / "README.md").write_text("rfig demo\n")
    subprocess.run(["git", "-C", str(work), "add", "."], check=True)
    subprocess.run(["git", "-C", str(work), "commit", "-qm", "initial"], check=True)
    for branch in ("develop", "feature/rfig-demo", "release/0.2"):
        subprocess.run(["git", "-C", str(work), "branch", branch], check=True)

    config = home / ".config/rfig"
    config.mkdir(parents=True)
    # Keep a directory-scoped history fixture for the final cancel state.
    (config / "history.tsv").write_text(f"{work}\x1fnpm run dev\n")
    # Seed usage so the showcase visibly demonstrates the circled ranking markers.
    (config / "usage.log").write_text(
        "C\x1fnpm run\x1fbuild\nC\x1fnpm run\x1fdev\nC\x1fbrew services\x1flist\n"
    )
    (config / "history.tsv").chmod(0o600)
    (config / "usage.log").chmod(0o600)

    snapshot = home / "snapshot"
    zshrc = home / ".zshrc"
    zshrc.write_text(
        "PROMPT='%F{magenta}>%f '\nRPROMPT=''\nbindkey -e\n"
        "autoload -Uz compinit; compinit -D\n"
        f"source {shlex.quote(str(ROOT / 'rfig.zsh'))}\n"
        "_demo_snapshot() { print -rl -- \"$BUFFER\" \"$_rfig_ghost\" \"$POSTDISPLAY\" > "
        f"{shlex.quote(str(snapshot))}" " }\n"
        "zle -N _demo_snapshot\nbindkey '^X' _demo_snapshot\n"
    )
    env = dict(
        os.environ,
        HOME=temp,
        ZDOTDIR=temp,
        XDG_CONFIG_HOME=str(home / ".config"),
        TERM="xterm-256color",
        LC_ALL="en_US.UTF-8",
        GIT_CONFIG_GLOBAL="/dev/null",
        GIT_CONFIG_NOSYSTEM="1",
        PATH=f"{ROOT / 'target/debug'}:/opt/homebrew/bin:/usr/bin:/bin",
    )
    tmux = [shutil.which("tmux"), "-L", f"rfig-showcase-{os.getpid()}", "-f", "/dev/null"]
    frames: list[Image.Image] = []
    durations: list[int] = []
    records: list[dict[str, str]] = []

    def call(*args: str) -> str:
        return subprocess.check_output(tmux + list(args), env=env, text=True, stderr=subprocess.STDOUT)

    def key(value: str, literal: bool = False) -> None:
        call("send-keys", "-t", "demo", *( ["-l"] if literal else []), value)

    def state() -> list[str]:
        snapshot.unlink(missing_ok=True)
        key("C-x")
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            if snapshot.exists():
                lines = snapshot.read_text().splitlines()
                if len(lines) >= 3:
                    return lines
            time.sleep(0.02)
        raise AssertionError("zsh snapshot timed out")

    def capture(title: str, duration: int = 900) -> list[str]:
        time.sleep(0.18)
        lines = state()
        ansi = call("capture-pane", "-p", "-e", "-t", "demo")
        cursor = tuple(map(int, call("display-message", "-p", "-t", "demo", "#{cursor_x} #{cursor_y}").split()))
        frames.append(render(ansi, cursor, title))
        durations.append(duration)
        records.append({"title": title, "buffer": lines[0], "ghost": lines[1], "screen": ansi})
        return lines

    def reset() -> None:
        key("C-u")
        key("C-l")
        time.sleep(0.15)

    try:
        call("new-session", "-d", "-s", "demo", "-c", str(work), "-x", "80", "-y", "12", "/bin/zsh", "-d", "-i")
        call("set-option", "-g", "status", "off")
        time.sleep(0.7)

        key("g", literal=True)
        capture("01 / git · type g", 850)
        key("i", literal=True)
        capture("01 / git · type gi", 850)
        key("t", literal=True)
        capture("01 / git · type git", 850)
        key(" checkout", literal=True)
        capture("01 / git · choose checkout", 1100)
        key("Right")
        capture("01 / git · dynamic branches", 1100)
        key("f", literal=True)
        capture("01 / git · filter the current repository", 1100)
        key("Right")
        key("Enter")
        time.sleep(0.25)
        capture("01 / git · command executed", 900)
        reset()

        key("brew services", literal=True)
        capture("02 / brew · subcommand menu", 900)
        key("Right")
        capture("02 / brew · service actions", 1100)
        key("l", literal=True)
        capture("02 / brew · usage-ranked option", 1000)
        reset()

        key("npm run", literal=True)
        capture("03 / npm · package scripts", 900)
        key("Right")
        capture("03 / npm · dynamic package.json candidates", 1200)
        key("Down")
        capture("03 / npm · ↓ switches the selection", 850)
        key("Right")
        capture("03 / npm · → inserts the selected script", 850)
        key("Enter")
        time.sleep(0.25)
        capture("03 / npm · selected script executed", 1000)
        reset()

        key("C-c")
        time.sleep(0.2)
        capture("04 / menu dismissed after cancel", 1100)
    finally:
        subprocess.run(tmux + ["kill-server"], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

frames[0].save(OUTPUT, save_all=True, append_images=frames[1:], duration=durations, loop=0, disposal=2, optimize=True)
(EVIDENCE / "recording.json").write_text(json.dumps(records, indent=2, ensure_ascii=False))
print(f"Recorded {len(frames)} frames, {sum(durations) / 1000:.2f}s: {OUTPUT}")
