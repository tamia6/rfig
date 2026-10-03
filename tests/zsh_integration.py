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


def wait_for_file(fd, path, timeout=5):
    deadline = time.monotonic() + timeout
    while not path.exists() and time.monotonic() < deadline:
        if select.select([fd], [], [], 0.05)[0]:
            output = os.read(fd, 65536)
            if b"\x1b[6n" in output:
                os.write(fd, CURSOR_REPLY)
    assert path.exists(), f"waiting for {path.name}"


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
    for name in ("rfiglatefixture", "rfiggenfixture", "rfiggroupfixture", "rfigshellfixture", "rfigfishfixture", "rfigbashfixture", "rfigprotocolfixture"):
        tool = home / "bin" / name
        if name == "rfiggenfixture":
            tool.write_text(
                '#!/bin/sh\n'
                'if [ "$1" = "--help" ]; then printf "Commands:\\n  completion  Generate completion\\n"; exit; fi\n'
                'if [ "$1" = "completion" ] && [ "$2" = "zsh" ]; then\n'
                '  printf "#compdef rfiggenfixture\\n_rfiggenfixture() { compadd generated-one generated-two; }\\ncompdef _rfiggenfixture rfiggenfixture\\n"; fi\n'
            )
        elif name == "rfiggroupfixture":
            tool.write_text(
                '#!/bin/sh\n'
                'if [ "$1" = "--help" ]; then printf "Basic Commands (Beginner):\\n  completion  Generate completion\\n"; exit; fi\n'
                'if [ "$1" = "completion" ] && [ "$2" = "zsh" ]; then\n'
                '  printf "#compdef rfiggroupfixture\\n_rfiggroupfixture() { compadd grouped-one grouped-two; }\\ncompdef _rfiggroupfixture rfiggroupfixture\\n"; fi\n'
            )
        elif name == "rfigshellfixture":
            tool.write_text(
                '#!/bin/sh\n'
                'if [ "$1" = "--help" ]; then printf "Commands:\\n  completion: Generate completion\\n"; exit; fi\n'
                'if [ "$1" = "completion" ] && [ "$2" = "zsh" ]; then printf "# bash completion\\n"; exit; fi\n'
                'if [ "$1" = "completion" ] && [ "$2" = "-s" ] && [ "$3" = "zsh" ]; then\n'
                '  printf "#compdef rfigshellfixture\\n_rfigshellfixture() { compadd shell-one shell-two; }\\ncompdef _rfigshellfixture rfigshellfixture\\n"; fi\n'
            )
        elif name == "rfigprotocolfixture":
            tool.write_text(
                '#!/bin/sh\n'
                'if [ "$1" = "--help" ]; then printf "Available Commands:\\n  run  Run\\nFlags:\\n  --help  Help\\n"; exit; fi\n'
                'if [ "$1" = "__complete" ]; then\n'
                '  if [ "$2" = "run" ]; then printf "nested-protocol\\n:0\\n"; '
                'else printf "protocol-one\\nprotocol-two\\n:0\\n"; fi; fi\n'
            )
        else:
            tool.write_text("#!/bin/sh\nexit 0\n")
        tool.chmod(0o755)
    late_dir = home / "late-fpath"
    late_dir.mkdir()
    (late_dir / "_rfiglatefixture").write_text("#compdef rfiglatefixture\n_rfiglatefixture() { compadd late-one late-two; }\n_rfiglatefixture \"$@\"\n")
    fish_dir = home / ".config/fish/completions"
    fish_dir.mkdir(parents=True)
    (fish_dir / "rfigfishfixture.fish").write_text(
        "complete -c rfigfishfixture -f -n 'not __fish_seen_subcommand_from fish-one' -a 'fish-one fish-two'\n"
        "complete -c rfigfishfixture -f -n '__fish_seen_subcommand_from fish-one' -a 'nested-fish'\n"
    )
    bash_dir = home / ".local/share/bash-completion/completions"
    bash_dir.mkdir(parents=True)
    (bash_dir / "rfigbashfixture").write_text(
        '_rfigbashfixture() { local choices="bash-one bash-two"; [[ ${COMP_WORDS[1]} == bash-one && $COMP_CWORD -gt 1 ]] && choices="nested-bash"; COMPREPLY=( $(compgen -W "$choices" -- "$2") ); }\n'
        'complete -F _rfigbashfixture rfigbashfixture\n'
    )
    for name, help_text in (
        ("rfighelpusage", "Usage:\\n  rfighelpusage setup   Configure\\n  rfighelpusage start   Start\\n"),
        ("rfighelpsection", "Available Commands:\\n  build   Build\\n  deploy  Deploy\\n"),
    ):
        fixture_help = home / "bin" / name
        fixture_help.write_text(f'#!/bin/sh\nprintf "{help_text}"\n')
        fixture_help.chmod(0o755)
    short_help = home / "bin/rfighelpshort"
    short_help.write_text(
        '#!/bin/sh\n'
        'if [ "$1" = "--help" ]; then printf "  --verbose  Print details\\n"; '
        'else printf "Commands:\\n  inspect  Inspect\\n  reset  Reset\\n"; fi\n'
    )
    short_help.chmod(0o755)
    branch_fixture = home / "bin/rfigbranchfixture"
    branch_fixture.write_text("#!/bin/sh\nexit 0\n")
    branch_fixture.chmod(0o755)
    option_help = home / "bin/rfigoptionhelp"
    option_help.write_text(
        '#!/bin/sh\n'
        'if [ "$1" = "--help" ]; then printf \'Use "rfigoptionhelp options" for a list of global options.\\n\'; '
        'elif [ "$1" = "options" ]; then printf "    --cache-dir=\x27\x27: cache path\\n    --verbose: verbose output\\n"; fi\n'
    )
    option_help.chmod(0o755)
    repo = home / "repo"
    repo.mkdir()
    subprocess.run(["git", "init", "-q", str(repo)], check=True)
    subprocess.run(["git", "-C", str(repo), "-c", "user.name=Test", "-c", "user.email=test@example.com", "commit", "-q", "--allow-empty", "-m", "init"], check=True)
    subprocess.run(["git", "-C", str(repo), "branch", "feature-rfig"], check=True)
    (repo / "tracked.txt").write_text("tracked\n")
    subprocess.run(["git", "-C", str(repo), "add", "tracked.txt"], check=True)
    subprocess.run(["git", "-C", str(repo), "-c", "user.name=Test", "-c", "user.email=test@example.com", "commit", "-q", "-m", "tracked"], check=True)
    (repo / "tracked.txt").write_text("modified\n")
    shutil.copy(ROOT / "target/debug/rfig", home / ".local/bin/rfig")
    (home / ".config/rfig").mkdir(parents=True)
    shutil.copy(ROOT / "rfig.zsh", home / ".config/rfig/rfig.zsh")
    setup_env = dict(os.environ, HOME=temp, SHELL="/bin/zsh", PATH=str(home / "bin"))
    for _ in range(2):
        subprocess.run([str(home / ".local/bin/rfig"), "setup"], env=setup_env, check=True)
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        status = home / ".config/rfig/enrich.status"
        if status.exists() and status.read_text().startswith("done\n"):
            break
        time.sleep(0.05)
    else:
        raise AssertionError("setup background enrichment did not finish")
    for name in ("rfiggenfixture", "rfiggroupfixture", "rfigshellfixture", "rfigprotocolfixture"):
        subprocess.run([str(home / ".local/bin/rfig"), "analyze", name], env=setup_env, check=True)
    generated = home / ".config/rfig/generated/rfiggenfixture.zsh"
    deadline = time.monotonic() + 10
    while not generated.exists() and time.monotonic() < deadline:
        time.sleep(0.02)
    assert generated.exists(), "explicit analysis must discover completion generators"
    for name in ("rfiggroupfixture", "rfigshellfixture"):
        expected = home / f".config/rfig/generated/{name}.zsh"
        deadline = time.monotonic() + 10
        while not expected.exists() and time.monotonic() < deadline:
            time.sleep(0.02)
        assert expected.exists(), f"analysis must discover the generator format used by {name}"
    protocol = home / ".config/rfig/protocol/rfigprotocolfixture"
    deadline = time.monotonic() + 10
    while not protocol.exists() and time.monotonic() < deadline:
        time.sleep(0.02)
    assert protocol.exists(), "explicit analysis must discover command completion protocols"
    assert "rfigfishfixture" in (home / ".config/rfig/commands.txt").read_text().splitlines()
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
        assert (brew_rc_dir / ".zshrc").read_text().strip() == f'source "{brew_home / ".config/rfig/rfig.zsh"}"'
        assert (brew_home / ".config/rfig/rfig.zsh").read_bytes() == (ROOT / "rfig.zsh").read_bytes()
        assert "brew" in (brew_home / ".config/rfig/commands.txt").read_text().splitlines()
    (home / ".zshrc").write_text(
        f"PROMPT=$'RFIG-TOP\\nRFIG> '\nautoload -Uz compinit; compinit -D\nfpath+=( {late_dir} )\nsource {ROOT / 'rfig.zsh'}\n"
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
        "_rfig_branch_values() { compadd feature-rfig; }\n"
        "_rfig_file_values() { compadd biz-center; }\n"
        "_rfig_branch_completion() { _alternative 'branches:branch:_rfig_branch_values' 'files:file:_rfig_file_values'; }\n"
        "compdef _rfig_branch_completion rfigbranchfixture\n"
        f"_rfig_snapshot() {{ print -rl -- \"${{_rfig_hits[@]}}\" > {home / 'hits'}; }}\n"
        "zle -N _rfig_snapshot\nbindkey '^X' _rfig_snapshot\n"
    )
    child_path=f"{home / '.local/bin'}:{home / 'bin'}:{os.environ['PATH']}"
    subprocess.run([str(home / ".local/bin/rfig"), "init"], env=dict(os.environ, HOME=temp, PATH=child_path), check=True)
    assert "git" in (home / ".config/rfig/supported.txt").read_text().splitlines()
    subprocess.run([str(home / ".local/bin/rfig"), "analyze", "rfigoptionhelp"], env=dict(os.environ, HOME=temp, PATH=child_path), check=True)
    subprocess.run([str(home / ".local/bin/rfig"), "analyze", "rfig"], env=dict(os.environ, HOME=temp, PATH=child_path), check=True)
    assert "--cache-dir\toption" in (home / ".config/rfig/options/rfigoptionhelp.tsv").read_text()
    for name, candidate in (("rfigfishfixture", "fish-two"), ("rfigbashfixture", "bash-two"), ("rfigprotocolfixture", "protocol-two")):
        response = subprocess.check_output([str(home / ".local/bin/rfig"), "external", name, name + " "], env=dict(os.environ, HOME=temp, PATH=child_path), text=True)
        assert response.startswith("provider\n") and candidate in response, (name, response)
    for name, branch, expected in (("rfigfishfixture", "fish-one", "nested-fish"), ("rfigbashfixture", "bash-one", "nested-bash"), ("rfigprotocolfixture", "run", "nested-protocol")):
        response = subprocess.check_output([str(home / ".local/bin/rfig"), "external", name, f"{name} {branch} "], env=dict(os.environ, HOME=temp, PATH=child_path), text=True)
        assert expected in response, (name, response)
    pid, fd = pty.fork()
    if pid == 0:
        os.environ.pop("NO_COLOR", None)
        os.environ.update(HOME=temp, ZDOTDIR=temp, TERM="xterm-256color", PATH=child_path)
        os.execv("/bin/zsh", ["zsh", "-i"])
    try:
        fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 100, 0, 0))
        wait_for(fd, b"RFIG> ")

        (home / "hits").unlink(missing_ok=True)
        os.write(fd, b"rfig\x18")
        wait_for_file(fd, home / "hits")
        assert any(row.startswith("analyze\t") for row in (home / "hits").read_text().splitlines())
        os.write(fd, b"\x15")

        for name, expected in (("rfiglatefixture", "late-two"), ("rfiggenfixture", "generated-two"), ("rfiggroupfixture", "grouped-two"), ("rfigshellfixture", "shell-two"), ("rfigprotocolfixture", "protocol-two"), ("rfigfishfixture", "fish-two"), ("rfigbashfixture", "bash-two")):
            (home / "hits").unlink(missing_ok=True)
            os.write(fd, name.encode())
            rendered = wait_for(fd, expected.encode())
            assert b"record=''" not in rendered, f"completion leaked a local variable for {name}"
            os.write(fd, b"\x18")
            wait_for_file(fd, home / "hits")
            assert any(row.startswith(expected + "\t") for row in (home / "hits").read_text().splitlines()), name
            os.write(fd, b"\x15")
            time.sleep(0.05)

        for name, branch, expected in (("rfigprotocolfixture", "run", "nested-protocol"), ("rfigfishfixture", "fish-one", "nested-fish"), ("rfigbashfixture", "bash-one", "nested-bash")):
            (home / "hits").unlink(missing_ok=True)
            os.write(fd, f"{name} {branch} ".encode())
            os.write(fd, b"\x18")
            wait_for_file(fd, home / "hits")
            assert any(row.startswith(expected + "\t") for row in (home / "hits").read_text().splitlines()), name
            os.write(fd, b"\x15")
            time.sleep(0.05)

        for typed in (b"rfigplain", b"rfigplain "):
            (home / "hits").unlink(missing_ok=True)
            os.write(fd, typed + b"\x18")
            wait_for_file(fd, home / "hits")
            hits = (home / "hits").read_text()
            assert hits == "\n", f"unsupported commands must not show unrelated matches: {hits[:250]!r}"
            os.write(fd, b"\x15")

        for command, expected, visible in (
            ("rfighelpusage", "setup", "start"),
            ("rfighelpsection", "build", "deploy"),
            ("rfighelpshort", "inspect", "reset"),
        ):
            (home / "hits").unlink(missing_ok=True)
            os.write(fd, command.encode())
            rendered = wait_for(fd, visible.encode())
            assert b"record=''" not in rendered, f"completion leaked a local variable for {command}"
            os.write(fd, b"\x18")
            deadline = time.monotonic() + 5
            while not (home / "hits").exists() and time.monotonic() < deadline:
                time.sleep(0.01)
            hits = (home / "hits").read_text().splitlines()
            assert any(line.startswith(expected + "\t") and line.endswith("\tsubcommand") for line in hits), hits
            if command == "rfighelpusage":
                os.write(fd, b"\x1b[C")
                wait_for(fd, b"RFIG> rfighelpusage setup")
            os.write(fd, b"\x15")

        (home / "hits").unlink(missing_ok=True)
        os.write(fd, b"cd")
        # Capture the full candidate list; src need not be one of the visible five.
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
        live = wait_for(fd, "→".encode())
        assert b"\x1b[7m" not in live, "selection must not use inverse video"
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
        assert any(line.startswith("--verbose\t") and line.endswith("\tflag") for line in hits.splitlines()), hits[:500]
        assert "-v\t-v\t\tflag" in hits, hits[:500]
        subprocess.run([str(home / ".local/bin/rfig"), "analyze", "rfigfixture"], env=dict(os.environ, HOME=temp, PATH=child_path), check=True)
        assert "--verbose\tflag" in (home / ".config/rfig/options/rfigfixture.tsv").read_text()
        (home / "hits").unlink()
        os.write(fd, b"x\x7f\x18")
        deadline = time.monotonic() + 5
        while not (home / "hits").exists() and time.monotonic() < deadline:
            time.sleep(0.01)
        assert any(line.startswith("--verbose\t") and line.endswith("\tflag") for line in (home / "hits").read_text().splitlines())
        os.write(fd, b"\x1b[B")
        pulse = wait_for(fd, "➜".encode())
        assert "➜".encode() in pulse, "selection must briefly enlarge and shift the arrow"
        os.write(fd, b"\x1b[C")
        selected_output = wait_for(fd, b"RFIG> rfigfixture zeta")
        assert b"\r\nRFIG-TOP" not in selected_output, "selection must stay on the existing prompt"
        os.write(fd, b"\r")
        wait_for(fd, b"RFIG> ")
        assert (home / "fixture-args").read_text() == "zeta\n"

        os.write(fd, b"rfigfixture")
        wait_for(fd, "→".encode())
        os.write(fd, b"\x1b[B")  # zeta was just used, so select alpha explicitly.
        time.sleep(0.1)
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

        os.write(fd, b"unfunction git\r")
        wait_for(fd, b"RFIG> ")
        os.write(fd, f"cd {repo}\r".encode())
        wait_for(fd, b"RFIG> ")
        (home / "hits").unlink(missing_ok=True)
        os.write(fd, b"git checkout ")
        wait_for(fd, "●".encode())
        os.write(fd, b"\x18")
        deadline = time.monotonic() + 5
        while not (home / "hits").exists() and time.monotonic() < deadline:
            time.sleep(0.01)
        checkout_hits = (home / "hits").read_text().splitlines()
        assert any(line.startswith("feature-rfig\t") and line.endswith("\targument") for line in checkout_hits), checkout_hits[:20]
        assert not any(line.startswith("tracked.txt\t") for line in checkout_hits), checkout_hits[:20]
        os.write(fd, b"\x15")

        (home / "hits").unlink(missing_ok=True)
        os.write(fd, b"git checkout -- ")
        wait_for(fd, "●".encode())
        os.write(fd, b"\x18")
        deadline = time.monotonic() + 5
        while not (home / "hits").exists() and time.monotonic() < deadline:
            time.sleep(0.01)
        assert any(line.startswith("tracked.txt\t") for line in (home / "hits").read_text().splitlines())
        os.write(fd, b"\x15")

        (repo / "biz-center").mkdir()
        completion_fixture = home / "checkout-completion.zsh"
        completion_fixture.write_text(
            "_rfig_git_branch() { compadd feature-rfig; }\n"
            "_rfig_git_file() { compadd biz-center; }\n"
            "_rfig_git_test() {\n"
            "  if [[ ${words[2]-} == checkout && $CURRENT == 3 ]]; then\n"
            "    _alternative 'branches:branch:_rfig_git_branch' 'files:file:_rfig_git_file'\n"
            "  else\n"
            "    _git \"$@\"\n"
            "  fi\n"
            "}\n"
            "compdef _rfig_git_test git\n"
        )
        os.write(fd, f"source {completion_fixture}\r".encode())
        wait_for(fd, b"RFIG> ")
        (home / "hits").unlink(missing_ok=True)
        os.write(fd, b"git checkout ")
        wait_for(fd, "→".encode())
        os.write(fd, b"\x18")
        deadline = time.monotonic() + 5
        while not (home / "hits").exists() and time.monotonic() < deadline:
            time.sleep(0.01)
        checkout_hits = (home / "hits").read_text().splitlines()
        assert any(line.startswith("feature-rfig\t") for line in checkout_hits), checkout_hits
        assert not any(line.startswith("biz-center\t") for line in checkout_hits), checkout_hits
        os.write(fd, b"\x15")

        (home / "hits").unlink(missing_ok=True)
        os.write(fd, b"rfigbranchfixture ")
        wait_for(fd, "●".encode())
        os.write(fd, b"\x18")
        wait_for_file(fd, home / "hits")
        branch_hits = (home / "hits").read_text().splitlines()
        assert any(line.startswith("feature-rfig\t") for line in branch_hits), branch_hits
        assert not any(line.startswith("biz-center\t") for line in branch_hits), branch_hits
        os.write(fd, b"\x15")

        if shutil.which("tmux"):
            tmux = ["tmux", "-L", f"rfig-test-{os.getpid()}", "-f", "/dev/null"]
            session_env = dict(os.environ, HOME=temp, ZDOTDIR=temp, PATH=child_path, TERM="xterm-256color")
            try:
                subprocess.run(tmux + ["new-session", "-d", "-s", "check", "-x", "100", "-y", "24", "/bin/zsh", "-i"], env=session_env, check=True)
                subprocess.run(tmux + ["send-keys", "-t", "check", "-l", "rfigdisplay"], env=session_env, check=True)
                deadline = time.monotonic() + 5
                while time.monotonic() < deadline:
                    screen = subprocess.check_output(tmux + ["capture-pane", "-t", "check", "-p"], env=session_env, text=True)
                    if "→ ● alpha" in screen:
                        break
                    time.sleep(0.05)
                assert "→ ● alpha" in screen, "the pointer must precede the argument icon"
                styled = subprocess.check_output(tmux + ["capture-pane", "-t", "check", "-p", "-e"], env=session_env)
                selected = next(line for line in styled.splitlines() if b"alpha" in line)
                assert b"\x1b[7m" not in selected, "the selected row must not use inverse video"
                assert b"\x1b[42m" not in selected, "the icon must not use a colored background"
                assert b"\x1b[32m" in selected, "the argument icon must use the terminal's green palette color"
                after_icon = selected.split("●".encode(), 1)[1].split(b"alpha", 1)[0]
                assert b"\x1b[36m" in after_icon, "the selected label must use the terminal's cyan palette color"
                subprocess.run(tmux + ["send-keys", "-t", "check", "Down"], env=session_env, check=True)
                deadline = time.monotonic() + 5
                while time.monotonic() < deadline:
                    screen = subprocess.check_output(tmux + ["capture-pane", "-t", "check", "-p"], env=session_env, text=True)
                    if "→ ● beta" in screen and "➜" not in screen:
                        break
                    time.sleep(0.02)
                assert "→ ● beta" in screen and "➜" not in screen, "animation must settle on the next item"
                styled = subprocess.check_output(tmux + ["capture-pane", "-t", "check", "-p", "-e"], env=session_env)
                selected = next(line for line in styled.splitlines() if b"beta" in line)
                assert b"\x1b[36m" in selected.split("●".encode(), 1)[1].split(b"beta", 1)[0], "the label color must follow selection"
                subprocess.run(tmux + ["send-keys", "-t", "check", "Up"], env=session_env, check=True)
                deadline = time.monotonic() + 5
                while time.monotonic() < deadline:
                    screen = subprocess.check_output(tmux + ["capture-pane", "-t", "check", "-p"], env=session_env, text=True)
                    if "→ ● alpha" in screen and "➜" not in screen:
                        break
                    time.sleep(0.02)
                assert "→ ● alpha" in screen and "➜" not in screen, "animation must settle on the previous item"
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
                for index, (typed, wanted, unwanted) in enumerate((
                    ("rfigfixture -", "-v", "--as"),
                    ("rfigfixture --", "--as", "-v"),
                )):
                    name = f"flags-{index}"
                    subprocess.run(tmux + ["new-session", "-d", "-s", name, "-x", "100", "-y", "24", "/bin/zsh", "-i"], env=session_env, check=True)
                    subprocess.run(tmux + ["send-keys", "-t", name, "-l", typed], env=session_env, check=True)
                    deadline = time.monotonic() + 5
                    menu = ""
                    while time.monotonic() < deadline:
                        screen = subprocess.check_output(tmux + ["capture-pane", "-t", name, "-p"], env=session_env, text=True)
                        if f"RFIG> {typed}" in screen:
                            menu = screen.rsplit(f"RFIG> {typed}", 1)[-1]
                            if "→" in menu:
                                break
                        time.sleep(0.05)
                    labels = []
                    for row in menu.splitlines():
                        parts = row.split()
                        if parts and parts[0] in {"→", "⌘", "↳", "●", "◇", "⚑"}:
                            labels.append(parts[-1])
                    assert wanted in labels and unwanted not in labels, (typed, labels)
                    subprocess.run(tmux + ["kill-session", "-t", name], env=session_env, check=True)
            finally:
                subprocess.run(tmux + ["kill-server"], env=session_env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        print("zsh live selection, nested completion, and terminal theme: OK")
    finally:
        os.close(fd)
        os.kill(pid, signal.SIGKILL)
        os.waitpid(pid, 0)
    for name, cached in (("rfiggenfixture", generated), ("rfigprotocolfixture", protocol)):
        stale_scripts = [home / f".config/rfig/generated/{name}.{shell}" for shell in ("bash", "fish")]
        for script in stale_scripts:
            script.write_text("# obsolete completion\n")
        (home / "bin" / name).write_text("#!/bin/sh\nexit 0\n")
        subprocess.run([str(home / ".local/bin/rfig"), "analyze", name], env=dict(os.environ, HOME=temp, PATH=child_path), check=True)
        assert not cached.exists(), f"stale completion cache for {name}"
        assert not any(script.exists() for script in stale_scripts), "all shells' stale generators must be removed"
