"""Record real zsh interactions into the README GIF.

Requires tmux, zsh, git, Pillow and pyte. Run from any directory:
    python3 scripts/record_demo.py
Uses a disposable HOME/repository; never reads the user's command history.
ANSI cells captured from tmux are rendered with a fixed terminal palette.
"""

import json
import os
import re
from pathlib import Path
import shlex
import shutil
import subprocess
import tempfile
import time

from PIL import Image, ImageDraw, ImageFont
import pyte

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "assets/rfig-demo.gif"
EVIDENCE = ROOT / "target/demo-recording"
EVIDENCE.mkdir(parents=True, exist_ok=True)
FONT = os.environ.get("RFIG_DEMO_FONT", "/System/Library/Fonts/Menlo.ttc")
font = ImageFont.truetype(FONT, 22)
small = ImageFont.truetype(FONT, 16)
symbols = ImageFont.truetype(os.environ.get("RFIG_DEMO_SYMBOL_FONT", "/System/Library/Fonts/Supplemental/Arial Unicode.ttf"), 22)
BG, FG = "#262633", "#d5d8ed"
COLORS = dict(zip(
    ["black", "red", "green", "brown", "blue", "magenta", "cyan", "white",
     "brightblack", "brightred", "brightgreen", "brightbrown", "brightblue",
     "brightmagenta", "brightcyan", "brightwhite"],
    ["#262633", "#ed8796", "#a6da95", "#eed49f", "#8aadf4", "#c6a0f6",
     "#8bd5ca", "#d5d8ed", "#9399b2", "#ed8796", "#a6da95", "#eed49f",
     "#8aadf4", "#c6a0f6", "#8bd5ca", "#ffffff"],
))
frames, durations, records = [], [], []


def color(value, default):
    if value == "default":
        return default
    return COLORS.get(value, "#" + value if len(value) == 6 else default)


def render(ansi, cursor, title):
    screen = pyte.Screen(62, 8)
    pyte.Stream(screen).feed(ansi.removesuffix("\n").replace("\n", "\r\n"))
    im = Image.new("RGB", (960, 376), "#101015")
    draw = ImageDraw.Draw(im)
    draw.rounded_rectangle((22, 18, 937, 357), radius=22, fill=BG, outline="#414152", width=2)
    draw.rounded_rectangle((24, 20, 935, 70), radius=20, fill="#30303d")
    draw.rectangle((24, 45, 935, 70), fill="#30303d")
    for x, fill in [(56, "#ff5f57"), (81, "#ffbd2e"), (106, "#28c840")]:
        draw.ellipse((x-7, 36, x+7, 50), fill=fill)
    draw.text((154, 32), title, font=small, fill="#b9bfd3")
    draw.text((817, 32), "rfig/zsh", font=small, fill="#9399b2")
    for row in range(8):
        for col in range(62):
            cell = screen.buffer[row][col]
            if not cell.data.strip():
                continue
            fg = color(cell.fg, FG)
            draw.text((48+col*14, 93+row*30), cell.data, font=symbols if cell.data in "①②③" else font, fill=fg,
                      stroke_width=0.25 if cell.bold else 0)
    x, y = cursor
    draw.line((48+x*14, 96+y*30, 48+x*14, 120+y*30), fill=FG, width=2)
    return im


with tempfile.TemporaryDirectory(prefix="rfig-demo-") as temp:
    home = Path(temp)
    work = home / "project"
    work.mkdir()
    (work / "src").mkdir()
    (work / "docs").mkdir()
    config = home / ".config/rfig"
    config.mkdir(parents=True)
    env = dict(os.environ, HOME=temp, ZDOTDIR=temp, XDG_CONFIG_HOME=str(home / '.config'),
               TERM="xterm-256color", LC_ALL="en_US.UTF-8", GIT_CONFIG_GLOBAL="/dev/null",
               GIT_CONFIG_NOSYSTEM="1", PATH=f"{ROOT / 'target/debug'}:/usr/bin:/bin")
    subprocess.run(["git", "init", "-q", "-b", "main", str(work)], env=env, check=True)
    # A known history fixture, independent of the host's history.
    history_command = "git status --short --branch"
    (config / "history.tsv").write_text(f"{work}\x1f{history_command}\n")
    (config / "history.tsv").chmod(0o600)
    snapshot = home / "snapshot"
    (home / ".zshrc").write_text(
        "PROMPT='%F{magenta}>%f '\nRPROMPT=''\nbindkey -e\n"
        "autoload -Uz compinit; compinit -D\n"
        f"source {shlex.quote(str(ROOT / 'rfig.zsh'))}\n"
        "_demo_snapshot() {\n"
        f"  print -rl -- \"$BUFFER\" \"$_rfig_ghost\" \"$POSTDISPLAY\" > {shlex.quote(str(snapshot))}\n"
        "}\nzle -N _demo_snapshot\nbindkey '^X' _demo_snapshot\n"
    )
    tmux = [shutil.which("tmux"), "-L", f"rfig-demo-{os.getpid()}", "-f", "/dev/null"]

    def call(*args):
        return subprocess.check_output(tmux + list(args), env=env, text=True)

    def key(value, literal=False):
        call("send-keys", "-t", "demo", *( ["-l"] if literal else []), value)

    def state():
        snapshot.unlink(missing_ok=True)
        key("C-x")
        until = time.monotonic() + 5
        while time.monotonic() < until:
            if snapshot.exists():
                lines = snapshot.read_text().splitlines()
                if len(lines) >= 2:
                    return lines
            time.sleep(0.02)
        raise AssertionError("zsh snapshot timed out")

    def capture(title, duration=800):
        time.sleep(0.18)
        lines = state()
        ansi = call("capture-pane", "-p", "-e", "-t", "demo")
        cursor = tuple(map(int, call("display-message", "-p", "-t", "demo", "#{cursor_x} #{cursor_y}").split()))
        frames.append(render(ansi, cursor, title))
        durations.append(duration)
        records.append(dict(title=title, buffer=lines[0], ghost=lines[1], screen=ansi))
        return lines

    def reset():
        key("C-u")
        key("C-l")
        time.sleep(0.15)

    try:
        call("new-session", "-d", "-s", "demo", "-c", str(work), "-x", "62", "-y", "8", "/bin/zsh", "-d", "-i")
        call("set-option", "-g", "status", "off")
        time.sleep(0.5)
        capture("01 / Command completion", 500)
        key("git stat", True)
        capture("01 / Type to filter the menu", 1200)
        key("Right")
        selected = capture("01 / Right arrow inserts the candidate", 1000)
        assert selected[0].strip() == "git status", selected
        key("Enter")
        time.sleep(0.3)
        capture("01 / Enter runs the command", 1200)
        reset()
        key("cd s", True)
        capture("02 / Paths from the working directory", 1000)
        key("Right")
        selected = capture("02 / Right arrow completes the path", 1000)
        assert selected[0].strip().rstrip("/") == "cd src", selected
        reset()
        # Earlier execution added a newer history entry; reload the fixture for this scene.
        (config / "history.tsv").write_text(f"{work}\x1f{history_command}\n")
        key("_rfig_history_load_dir", True)
        key("Enter")
        time.sleep(0.2)
        reset()
        key("g", True)
        capture("03 / History-based autosuggestions", 250)
        key("i", True)
        suggested = capture("03 / Only 'gi' is typed; the rest is suggested", 2400)
        assert suggested[0] == "gi" and suggested[1] == "t status --short --branch", suggested
        assert "git" in "\n".join(suggested[3:]), suggested
        assert "status" not in "\n".join(suggested[3:]), suggested
        frames[-1].save(EVIDENCE / "history-suggestion.png")
        key("C-e")
        accepted = capture("03 / Ctrl+E accepts without executing", 1800)
        assert accepted[0] == history_command and not accepted[1], accepted
        frames[-1].save(EVIDENCE / "history-accepted.png")
        key("Enter")
        time.sleep(0.3)
        capture("03 / Enter executes the accepted command", 1300)
        reset()
        key("gi", True)
        suggested = capture("04 / Or execute the suggestion directly", 1800)
        assert suggested[0] == "gi" and suggested[1] == "t status --short --branch", suggested
        key("Enter")
        time.sleep(0.3)
        capture("04 / Enter runs the full suggestion", 1800)
        plain = re.sub(r'\x1b\[[0-9;]*m', '', records[-1]['screen'])
        assert history_command in plain and '## No commits yet on main' in plain, records[-1]
        frames[-1].save(EVIDENCE / "history-executed.png")
    finally:
        subprocess.run(tmux + ["kill-server"], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

frames[0].save(OUTPUT, save_all=True, append_images=frames[1:], duration=durations, loop=0, disposal=2, optimize=True)
(EVIDENCE / "recording.json").write_text(json.dumps(records, indent=2, ensure_ascii=False))
print(f"Recorded {len(frames)} frames, {sum(durations)/1000:.2f}s: {OUTPUT}")
