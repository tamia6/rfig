"""Check persistent, context-specific ranking in an interactive zsh."""

import os
from pathlib import Path
import subprocess
import tempfile
import time
from process_cleanup import cleanup_tmux


ROOT = Path(__file__).resolve().parents[1]
SEP = "\x1f"


with tempfile.TemporaryDirectory() as temp:
    home = Path(temp)
    config = home / ".config/rfig"
    config.mkdir(parents=True)
    usage = config / "usage.log"
    work = home / "work"
    work.mkdir()
    ranked_names = ("alpha", "bravo two", "charlie", "delta", "echo", "foxtrot", "golf", "hotel")
    # Force a middle insertion with multi-element slices, then score more candidates.
    ranked_weights = (4, 3, 0, 0, 2, 1, 2, 0)
    for name in ranked_names:
        (work / name).mkdir()
    usage.write_text(
        (f"C{SEP}rfigfixture{SEP}beta\n" * 3)
        + "".join(f"C{SEP}rfigpopular{SEP}{name}\n" * count for name, count in (("alpha", 4), ("beta", 3), ("gamma", 2), ("delta", 1)))
        + (f"C{SEP}rfigmixed{SEP}--verbose\n" * 3)
        + f"C{SEP}rfigmixed{SEP}build\n"
        + (f"C{SEP}other{SEP}alpha\n" * 20)
        + "".join(
            f"C{SEP}{command}{SEP}{name}\n" * weight
            for command in ("cd", "ls")
            for name, weight in zip(ranked_names, ranked_weights)
        )
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
        "_rfig_snapshot() { printf '%s\\0' \"$BUFFER\" \"$#_rfig_hits\" \"${_rfig_hits[@]}\" done > \"$HOME/snapshot\"; }\n"
        "zle -N _rfig_snapshot\nbindkey '^X' _rfig_snapshot\n"
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
        wait_for(lambda output: output.rstrip().splitlines()[-1] == "RFIG>")
        failures = []
        for command in ("cd", "ls"):
            snapshot = home / "snapshot"
            snapshot.unlink(missing_ok=True)
            line = f"{command} {work}/"
            call("send-keys", "-t", "usage", "-l", line)
            call("send-keys", "-t", "usage", "C-x")
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline:
                data = snapshot.read_bytes() if snapshot.exists() else b""
                if data.endswith(b"done\0"):
                    break
                time.sleep(0.04)
            assert data.endswith(b"done\0"), screen()
            rows = data.decode().split("\0")
            records = [row.split("\t") for row in rows[2:-2]]
            expected = [name for name, _ in sorted(zip(ranked_names, ranked_weights), key=lambda item: -item[1])]
            output = call("capture-pane", "-t", "usage", "-p", "-S", "-100")
            if (rows[0] != line or rows[1] != str(len(expected)) or [row[0] for row in records] != expected
                    or any(len(row) != 4 for row in records) or "bad math expression" in output):
                failures.append((command, rows, output))
            else:
                print(f"{command}: all {len(records)} records intact; score order and stable ties: OK")
            call("send-keys", "-t", "usage", "C-u")
        assert not failures, failures
    finally:
        cleanup_tmux(tmux, [home])

print("zsh usage ranking and recording: OK")
