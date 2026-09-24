"""End-to-end zsh check; run after `cargo build` with `python3 tests/zsh_integration.py`."""

import fcntl
import os
import pathlib
import pty
import select
import shutil
import signal
import struct
import subprocess
import tempfile
import termios
import time


ROOT = pathlib.Path(__file__).resolve().parents[1]
CURSOR_REPLY = b"\x1b[2;10R"


def wait_for(fd, needle, timeout=5):
    output = b""
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if select.select([fd], [], [], 0.1)[0]:
            chunk = os.read(fd, 65536)
            output += chunk
            # A PTY has no terminal emulator, so answer crossterm's cursor query.
            if b"\x1b[6n" in chunk:
                os.write(fd, CURSOR_REPLY)
            if needle in output:
                return output
    raise AssertionError(f"waiting for {needle!r}; output ended with {output[-300:]!r}")


with tempfile.TemporaryDirectory() as temp:
    home = pathlib.Path(temp)
    (home / ".local/bin").mkdir(parents=True)
    (home / "bin").mkdir()
    fixture = home / "bin/rfigfixture"
    fixture.write_text(
        '#!/bin/sh\n'
        'if [ "$1" = "--help" ]; then\n'
        '  printf "    --as string  Username\\n    --cache-dir string  Cache directory\\n    --verbose  Print details\\n"\n'
        '  exit 0\n'
        'fi\n'
        f'printf "%s\\n" "$*" > "{home / "fixture-args"}"\n'
    )
    fixture.chmod(0o755)
    plain = home / "bin/rfigplain"
    plain.write_text("#!/bin/sh\nexit 0\n")
    plain.chmod(0o755)
    repo = home / "repo"
    repo.mkdir()
    subprocess.run(["git", "init", "-q", str(repo)], check=True)
    subprocess.run(["git", "-C", str(repo), "-c", "user.name=Test", "-c", "user.email=test@example.com", "commit", "-q", "--allow-empty", "-m", "init"], check=True)
    subprocess.run(["git", "-C", str(repo), "branch", "feature-rfig"], check=True)
    shutil.copy(ROOT / "target/debug/rfig", home / ".local/bin/rfig")
    (home / ".config/rfig").mkdir(parents=True)
    shutil.copy(ROOT / "rfig.zsh", home / ".config/rfig/rfig.zsh")
    setup_env = dict(os.environ, HOME=temp, SHELL="/bin/zsh", PATH=str(home / "bin"))
    for _ in range(2):
        subprocess.run([str(home / ".local/bin/rfig"), "setup"], env=setup_env, check=True)
    assert (home / ".zshrc").read_text().count('source "' + str(home / ".config/rfig/rfig.zsh") + '"') == 1
    assert "rfigfixture" in (home / ".config/rfig/commands.txt").read_text().splitlines()
    with tempfile.TemporaryDirectory() as brew_temp:
        brew_home = pathlib.Path(brew_temp)
        brew_bin = brew_home / "bin"
        brew_bin.mkdir()
        brew_prefix = brew_home / "opt/rfig"
        (brew_prefix / "share/rfig").mkdir(parents=True)
        shutil.copy(ROOT / "rfig.zsh", brew_prefix / "share/rfig/rfig.zsh")
        (brew_prefix / "bin").mkdir()
        shutil.copy(ROOT / "target/debug/rfig", brew_prefix / "bin/rfig")
        brew = brew_bin / "brew"
        brew.write_text(f'#!/bin/sh\nprintf "%s\\n" "{brew_prefix}"\n')
        brew.chmod(0o755)
        brew_rc_dir = brew_home / "zsh"
        brew_rc_dir.mkdir()
        brew_env = dict(os.environ, HOME=brew_temp, ZDOTDIR=str(brew_rc_dir), SHELL="/bin/zsh", PATH=str(brew_bin))
        subprocess.run([str(brew_prefix / "bin/rfig"), "setup"], env=brew_env, check=True)
        assert (brew_rc_dir / ".zshrc").read_text().strip() == f'source "{brew_prefix / "share/rfig/rfig.zsh"}"'
    (home / ".zshrc").write_text(
        f"PROMPT=$'RFIG-TOP\\nRFIG> '\nautoload -Uz compinit; compinit -D\nsource {ROOT / 'rfig.zsh'}\n"
        f"git() {{ print -r -- \"$*\" > {home / 'args'}; }}\n"
        "_rfigfixture() {\n"
        "  if (( CURRENT == 2 )); then\n"
        "    local -a commands=( 'alpha:first' 'zeta:second' )\n"
        "    _describe -t subcommands 'subcommand' commands\n"
        "    compadd -- --as --cache-dir --env= --verbose -v\n"
        "  elif (( CURRENT == 3 )) && [[ $words[2] == alpha ]]; then compadd beta\n"
        "  elif (( CURRENT == 4 )) && [[ $words[3] == beta ]]; then compadd gamma\n"
        "  fi\n}\ncompdef _rfigfixture rfigfixture\n"
        "rfigdisplay() { print RFIG_RESULT; }\n"
        "_rfigdisplay_completions() { compadd alpha beta; }\n"
        "compdef _rfigdisplay_completions rfigdisplay\n"
        f"_rfig_snapshot() {{ print -rl -- \"${{_rfig_hits[@]}}\" > {home / 'hits'}; }}\n"
        "zle -N _rfig_snapshot\nbindkey '^X' _rfig_snapshot\n"
    )
    child_path=f"{home / 'bin'}:{os.environ['PATH']}"
    subprocess.run([str(home / ".local/bin/rfig"), "init"], env=dict(os.environ, HOME=temp, PATH=child_path), check=True)
    assert "git" in (home / ".config/rfig/supported.txt").read_text().splitlines()
    pid, fd = pty.fork()
    if pid == 0:
        os.environ.pop("NO_COLOR", None)
        os.environ.update(HOME=temp, ZDOTDIR=temp, TERM="xterm-256color", PATH=child_path)
        os.execv("/bin/zsh", ["zsh", "-i"])
    try:
        fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 100, 0, 0))
        wait_for(fd, b"RFIG> ")

        for typed in (b"rfigplain", b"rfigplain "):
            (home / "hits").unlink(missing_ok=True)
            os.write(fd, typed + b"\x18")
            deadline = time.monotonic() + 5
            while not (home / "hits").exists() and time.monotonic() < deadline:
                time.sleep(0.01)
            hits = (home / "hits").read_text()
            assert hits == "\n", f"unsupported commands must not show unrelated matches: {hits[:250]!r}"
            os.write(fd, b"\x15")

        (home / "hits").unlink(missing_ok=True)
        os.write(fd, b"cd")
        wait_for(fd, b"src")
        os.write(fd, b"\x18")
        deadline = time.monotonic() + 5
        while not (home / "hits").exists() and time.monotonic() < deadline:
            time.sleep(0.01)
        assert any(line.startswith("src\t") and line.endswith("\targument") for line in (home / "hits").read_text().splitlines())
        os.write(fd, b"\x15")

        os.write(fd, b"rfigfixture\r")
        wait_for(fd, b"RFIG> ")
        assert (home / "fixture-args").read_text() == "\n", "Enter must run the typed command"

        (home / "hits").unlink(missing_ok=True)
        os.write(fd, b"rfigfi\x18")
        deadline = time.monotonic() + 5
        while not (home / "hits").exists() and time.monotonic() < deadline:
            time.sleep(0.01)
        assert any(line.startswith("rfigfixture\t") and line.endswith("\tcommand") for line in (home / "hits").read_text().splitlines())
        os.write(fd, b"xture")
        live = wait_for(fd, b"\x1b[7m")
        assert b"48;5;25m" not in live, "selection must use the terminal theme"
        (home / "hits").unlink(missing_ok=True)
        os.write(fd, b"\x18")
        deadline = time.monotonic() + 5
        while not (home / "hits").exists() and time.monotonic() < deadline:
            time.sleep(0.01)
        hits = (home / "hits").read_text()
        assert any(line.startswith("alpha\t") and line.endswith("\tsubcommand") for line in hits.splitlines()), hits[:500]
        assert any(line.startswith("--as\t") and line.endswith("\toption") for line in hits.splitlines()), hits[:500]
        assert any(line.startswith("--cache-dir\t") and line.endswith("\toption") for line in hits.splitlines()), hits[:500]
        assert any(line.startswith("--env=\t") and line.endswith("\toption") for line in hits.splitlines()), hits[:500]
        assert any(line.startswith("--verbose\t") and line.endswith("\toption") for line in hits.splitlines()), hits[:500]
        assert "-v\t-v\t\tflag" in hits, hits[:500]
        subprocess.run([str(home / ".local/bin/rfig"), "analyze", "rfigfixture"], env=dict(os.environ, HOME=temp, PATH=child_path), check=True)
        assert "--verbose\tflag" in (home / ".config/rfig/options/rfigfixture.tsv").read_text()
        (home / "hits").unlink()
        os.write(fd, b"x\x7f\x18")
        deadline = time.monotonic() + 5
        while not (home / "hits").exists() and time.monotonic() < deadline:
            time.sleep(0.01)
        assert any(line.startswith("--verbose\t") and line.endswith("\tflag") for line in (home / "hits").read_text().splitlines())
        os.write(fd, b"\x1b[B\x1b[C")
        selected_output = wait_for(fd, b"RFIG> rfigfixture zeta")
        assert b"\r\nRFIG-TOP" not in selected_output, "selection must stay on the existing prompt"
        os.write(fd, b"\r")
        wait_for(fd, b"RFIG> ")
        assert (home / "fixture-args").read_text() == "zeta\n"

        os.write(fd, b"rfigfixture")
        wait_for(fd, b"\x1b[7m")
        os.write(fd, b"\x1b[C")
        wait_for(fd, b"RFIG> rfigfixture alpha")
        (home / "hits").unlink(missing_ok=True)
        os.write(fd, b"\x18")
        deadline = time.monotonic() + 5
        while not (home / "hits").exists() and time.monotonic() < deadline:
            time.sleep(0.01)
        assert any(line.startswith("beta\t") and line.endswith("\targument") for line in (home / "hits").read_text().splitlines())
        os.write(fd, b"\x1b[C")
        wait_for(fd, b"RFIG> rfigfixture alpha beta")
        os.write(fd, b"\x1b[C")
        wait_for(fd, b"RFIG> rfigfixture alpha beta gamma")
        os.write(fd, b"\r")
        wait_for(fd, b"RFIG> ")
        assert (home / "fixture-args").read_text() == "alpha beta gamma\n"

        os.write(fd, b"git br\x1b[C")
        wait_for(fd, b"RFIG> git branch")
        os.write(fd, b"x\r")
        wait_for(fd, b"RFIG> ")
        assert (home / "args").read_text() == "branch x\n"

        if shutil.which("tmux"):
            tmux = ["tmux", "-L", f"rfig-test-{os.getpid()}", "-f", "/dev/null"]
            session_env = dict(os.environ, HOME=temp, ZDOTDIR=temp, PATH=child_path, TERM="xterm-256color")
            try:
                subprocess.run(tmux + ["new-session", "-d", "-s", "check", "-x", "100", "-y", "24", "/bin/zsh", "-i"], env=session_env, check=True)
                subprocess.run(tmux + ["send-keys", "-t", "check", "-l", "rfigdisplay"], env=session_env, check=True)
                deadline = time.monotonic() + 5
                while time.monotonic() < deadline:
                    screen = subprocess.check_output(tmux + ["capture-pane", "-t", "check", "-p"], env=session_env, text=True)
                    if " ● alpha" in screen:
                        break
                    time.sleep(0.05)
                assert " ● alpha" in screen, "the preview must show the argument icon"
                styled = subprocess.check_output(tmux + ["capture-pane", "-t", "check", "-p", "-e"], env=session_env)
                selected = next(line for line in styled.splitlines() if b"alpha" in line)
                before_label = selected[selected.index(b"\x1b[7m"):selected.index(b"alpha")]
                assert b"\x1b[0m" not in before_label, "the selected row must stay highlighted across its icon"
                assert b"\x1b[42m" in before_label, "the selected argument icon must use the terminal's green palette color"
                subprocess.run(tmux + ["send-keys", "-t", "check", "Right"], env=session_env, check=True)
                deadline = time.monotonic() + 5
                while time.monotonic() < deadline:
                    screen = subprocess.check_output(tmux + ["capture-pane", "-t", "check", "-p"], env=session_env, text=True)
                    if "RFIG> rfigdisplay alpha" in screen:
                        break
                    time.sleep(0.05)
                assert "RFIG> rfigdisplay alpha" in screen
                assert " ● alpha" not in screen, "choosing a candidate must hide an unchanged menu"
                subprocess.run(tmux + ["send-keys", "-t", "check", "Enter"], env=session_env, check=True)
                deadline = time.monotonic() + 5
                while time.monotonic() < deadline:
                    screen = subprocess.check_output(tmux + ["capture-pane", "-t", "check", "-p"], env=session_env, text=True)
                    if "RFIG_RESULT" in screen:
                        break
                    time.sleep(0.05)
                assert "RFIG_RESULT" in screen
                assert " ● alpha" not in screen, "the preview must disappear before command output"
                lines = screen.splitlines()
                command_line = next(i for i, line in enumerate(lines) if "RFIG> rfigdisplay" in line)
                result_line = next(i for i, line in enumerate(lines) if "RFIG_RESULT" in line)
                assert result_line == command_line + 1, "the preview must leave no blank lines before output"
            finally:
                subprocess.run(tmux + ["kill-server"], env=session_env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        print("zsh live selection, nested completion, and terminal theme: OK")
    finally:
        os.kill(pid, signal.SIGTERM)
        os.close(fd)
