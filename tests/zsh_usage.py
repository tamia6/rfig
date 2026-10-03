"""Check persistent, context-specific ranking in an interactive zsh."""

import os
from pathlib import Path
import subprocess
import tempfile
import time


ROOT = Path(__file__).resolve().parents[1]
SEP = "\x1f"


with tempfile.TemporaryDirectory() as temp:
    home = Path(temp)
    config = home / ".config/rfig"
    config.mkdir(parents=True)
    usage = config / "usage.log"
    usage.write_text(
        (f"C{SEP}rfigfixture{SEP}beta\n" * 3)
        + "".join(f"C{SEP}rfigpopular{SEP}{name}\n" * count for name, count in (("alpha", 4), ("beta", 3), ("gamma", 2), ("delta", 1)))
        + (f"C{SEP}rfigmixed{SEP}--verbose\n" * 3)
        + f"C{SEP}rfigmixed{SEP}build\n"
        + (f"C{SEP}other{SEP}alpha\n" * 20)
    )
    (home / ".zshrc").write_text(
        "PROMPT='RFIG> '\n"
        "autoload -Uz compinit; compinit -D\n"
        f"source {ROOT / 'rfig.zsh'}\n"
        f"rfigfixture() {{ print -r -- \"$*\" > {home / 'result'}; }}\n"
        "_rfigfixture() { compadd alpha beta; }\n"
        "compdef _rfigfixture rfigfixture\n"
        "rfigpopular() { :; }\n"
        "_rfigpopular() { compadd alpha beta gamma delta epsilon; }\n"
        "compdef _rfigpopular rfigpopular\n"
        "rfigmixed() { :; }\n"
        "_rfigmixed() { compadd -- build deploy --verbose --help; }\n"
        "compdef _rfigmixed rfigmixed\n"
    )
    socket = f"rfig-usage-{os.getpid()}"
    tmux = ["tmux", "-L", socket, "-f", "/dev/null"]
    env = dict(os.environ, HOME=temp, ZDOTDIR=temp, TERM="xterm-256color")

    def call(*args):
        return subprocess.check_output(tmux + list(args), env=env, text=True)

    def screen():
        return call("capture-pane", "-t", "usage", "-p")

    def wait_for(predicate):
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            output = screen()
            if predicate(output):
                return output
            time.sleep(0.04)
        raise AssertionError(f"timed out; screen: {output!r}")

    try:
        call("new-session", "-d", "-s", "usage", "-x", "80", "-y", "20", "/bin/zsh", "-i")
        call("set-option", "-g", "status", "off")
        wait_for(lambda output: "RFIG>" in output)
        call("send-keys", "-t", "usage", "-l", "rfigpopular")
        output = wait_for(lambda output: "→ ① alpha" in output and "② beta" in output and "③ gamma" in output)
        assert "● delta" in output and "● epsilon" in output, output
        call("send-keys", "-t", "usage", "C-u")
        call("send-keys", "-t", "usage", "-l", "rfigmixed")
        output = wait_for(lambda output: "→ ① --verbose" in output and "② build" in output)
        assert "◇ --help" in output and "③ --help" not in output, output
        call("send-keys", "-t", "usage", "C-u")
        call("send-keys", "-t", "usage", "-l", "rfigfixture")
        output = wait_for(lambda output: "→ ① beta" in output)
        rows = output.splitlines()
        assert next(row for row in rows if "→ ①" in row).strip().endswith("beta"), rows
        assert "● alpha" in output and "② alpha" not in output, rows
        call("send-keys", "-t", "usage", "Down")
        wait_for(lambda output: "→ ● alpha" in output and "➜" not in output)
        call("send-keys", "-t", "usage", "Right")
        wait_for(lambda output: "RFIG> rfigfixture alpha" in output)
        call("send-keys", "-t", "usage", "Enter")
        deadline = time.monotonic() + 5
        while not (home / "result").exists() and time.monotonic() < deadline:
            time.sleep(0.04)
        assert (home / "result").read_text() == "alpha\n"
        events = usage.read_text().splitlines()
        assert f"C{SEP}rfigfixture{SEP}alpha" in events, events
        assert f"R{SEP}rfigfixture{SEP}alpha" in events, events
        for _ in range(2):
            wait_for(lambda output: output.rstrip().splitlines()[-1] == "RFIG>")
            call("send-keys", "-t", "usage", "-l", "rfigfixture")
            wait_for(lambda output: "→ ① beta" in output)
            call("send-keys", "-t", "usage", "Down")
            wait_for(lambda output: "→ ② alpha" in output and "➜" not in output)
            call("send-keys", "-t", "usage", "Right", "Enter")
            time.sleep(0.1)
        call("kill-session", "-t", "usage")
        call("new-session", "-d", "-s", "usage", "-x", "80", "-y", "20", "/bin/zsh", "-i")
        wait_for(lambda output: "RFIG>" in output)
        call("send-keys", "-t", "usage", "-l", "rfigfixture")
        wait_for(lambda output: "→ ① alpha" in output and "② beta" in output)
        call("send-keys", "-t", "usage", "C-c")
        call("send-keys", "-t", "usage", "-l", "rfigfixture beta")
        call("send-keys", "-t", "usage", "Enter")
        deadline = time.monotonic() + 5
        while (home / "result").read_text() != "beta\n" and time.monotonic() < deadline:
            time.sleep(0.04)
        assert (home / "result").read_text() == "beta\n"
        assert f"R{SEP}rfigfixture{SEP}beta" in usage.read_text().splitlines()
    finally:
        subprocess.run(tmux + ["kill-server"], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

print("zsh usage ranking and recording: OK")
