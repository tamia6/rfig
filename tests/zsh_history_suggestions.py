"""Check rfig's built-in history suggestions in an interactive zsh."""

import os
from pathlib import Path
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory() as temp:
        home = Path(temp)
        work_a = home / "work-a"
        work_b = home / "work-b"
        work_a.mkdir()
        work_b.mkdir()
        history_file = home / ".config/rfig/history.tsv"
        history_file.parent.mkdir(parents=True)
        history_file.write_text(
            f"{work_a}\x1fplainfixture hello\n"
            f"{work_a}\x1frfigfixture beta\n"
            f"{work_a}\x1fgit checkout master\n"
            f"{work_b}\x1fgit switch develop\n"
        )
        (home / ".zshrc").write_text(
            "PROMPT='RFIG> '\nautoload -Uz compinit; compinit -D\n"
            "plainfixture() { :; }\nrfigfixture() { :; }\n"
            f"git() {{ print -r -- \"$*\" > {home / 'executed'}; }}\n"
            "_rfigfixture() { compadd alpha beta; }\ncompdef _rfigfixture rfigfixture\n"
            "print -s 'git checkout wrong-zsh-history'\n"
            + f"source {ROOT / 'rfig.zsh'}\n"
            + f"_rfig_snapshot() {{ print -r -- \"post=$POSTDISPLAY\" \"hits=$#_rfig_hits\" \"buffer=$BUFFER\" \"widget=$widgets[self-insert]\" > {home / 'snapshot'}; }}\n"
            "zle -N _rfig_snapshot\nbindkey '^X' _rfig_snapshot\n"
        )
        socket = f"rfig-history-{os.getpid()}"
        tmux = ["tmux", "-L", socket, "-f", "/dev/null"]
        env = dict(os.environ, HOME=temp, ZDOTDIR=temp, TERM="xterm-256color")

        def call(*args):
            return subprocess.check_output(tmux + list(args), env=env, text=True)

        def snapshot_for(value):
            call("send-keys", "-t", "compat", "-l", value)
            return snapshot_current()

        def snapshot_current():
            (home / "snapshot").unlink(missing_ok=True)
            call("send-keys", "-t", "compat", "C-x")
            deadline = time.monotonic() + 6
            while time.monotonic() < deadline:
                if (home / "snapshot").exists():
                    return (home / "snapshot").read_text()
                time.sleep(0.04)
            raise AssertionError("no ZLE snapshot")

        try:
            call("new-session", "-d", "-s", "compat", "-c", str(work_a), "-x", "85", "-y", "20", "/bin/zsh", "-i")
            call("set-option", "-g", "status", "off")
            time.sleep(0.3)
            plain = snapshot_for("plainfixture h")
            assert "post=ello" in plain and "hits=0" in plain, plain
            call("send-keys", "-t", "compat", "Left")
            moved = snapshot_current()
            assert "post= hits=0" in moved, moved
            call("send-keys", "-t", "compat", "C-e")
            resumed = snapshot_current()
            assert "post=ello" in resumed and "buffer=plainfixture h" in resumed, resumed
            call("send-keys", "-t", "compat", "Right")
            screen = call("capture-pane", "-t", "compat", "-p")
            assert "RFIG> plainfixture hello" in screen, screen
            call("send-keys", "-t", "compat", "C-u")
            menu = snapshot_for("rfigfixture")
            assert "post= beta\n" in menu and "alpha" in menu and "hits=2" in menu, menu
            call("send-keys", "-t", "compat", "Right")
            screen = call("capture-pane", "-t", "compat", "-p")
            assert "rfigfixture alpha" in screen, screen
            call("send-keys", "-t", "compat", "C-u")
            snapshot_for("rfigfixture")
            call("send-keys", "-t", "compat", "C-e")
            screen = call("capture-pane", "-t", "compat", "-p")
            assert "RFIG> rfigfixture beta" in screen, screen
            call("send-keys", "-t", "compat", "C-u")
            short = snapshot_for("gi")
            assert "post=t checkout master\n" in short and "hits=0" not in short, short
            assert "checkout" not in "\n".join(short.splitlines()[1:]), short
            call("send-keys", "-t", "compat", "Enter")
            deadline = time.monotonic() + 6
            while not (home / "executed").exists() and time.monotonic() < deadline:
                time.sleep(0.04)
            assert (home / "executed").read_text() == "checkout master\n"
            assert f"{work_a}\x1fgit checkout master\n" in history_file.read_text()
            call("send-keys", "-t", "compat", "-l", f"cd {work_b}")
            call("send-keys", "-t", "compat", "Enter")
            time.sleep(0.2)
            second_dir = snapshot_for("gi")
            assert "post=t switch develop\n" in second_dir, second_dir
            assert "checkout" not in "\n".join(second_dir.splitlines()[1:]), second_dir
            call("send-keys", "-t", "compat", "C-u")
            missing = snapshot_for("plainfixture h")
            assert "post= hits=0" in missing, missing
            call("send-keys", "-t", "compat", "C-u")
            call("send-keys", "-t", "compat", "-l", "plainfixture unique")
            call("send-keys", "-t", "compat", "Enter")
            deadline = time.monotonic() + 5
            while f"{work_b}\x1fplainfixture unique\n" not in history_file.read_text() and time.monotonic() < deadline:
                time.sleep(0.04)
            assert f"{work_b}\x1fplainfixture unique\n" in history_file.read_text(), history_file.read_text()
            learned = snapshot_for("plainfixture u")
            assert "post=nique" in learned, learned
        finally:
            subprocess.run(tmux + ["kill-server"], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

print("rfig built-in history suggestions: OK")
