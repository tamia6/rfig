"""rfig's own public CLI participates in the generic zsh completion pipeline."""

import os
import pathlib
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
BINARY = ROOT / "target/debug/rfig"

help_text = subprocess.check_output([str(BINARY), "--help"], text=True)
assert subprocess.check_output([str(BINARY), "-h"], text=True) == help_text
assert help_text.startswith("Usage: rfig ")
assert "Commands:" in help_text and "Options:" in help_text and "Examples:" in help_text
for command in ("setup", "init", "analyze", "completion"):
    assert command in help_text, command
for internal in ("enrich-background", "external", "position", "pick"):
    assert internal not in help_text, internal
script = subprocess.check_output([str(BINARY), "completion", "zsh"], text=True)
assert script.startswith("#compdef rfig\n")
with tempfile.TemporaryDirectory() as temp:
    home = pathlib.Path(temp)
    config = home / ".config/rfig"
    setup_help = subprocess.run([str(BINARY), "setup", "--help"], env=dict(os.environ, HOME=temp, SHELL="/bin/zsh"), capture_output=True, text=True)
    assert setup_help.returncode == 0 and setup_help.stdout == help_text
    assert not config.exists(), "asking for setup help must not run setup"
    config.mkdir(parents=True)
    (config / "commands.txt").write_text("git\nmock-tool\n")
    bin_dir = home / "bin"
    bin_dir.mkdir()
    (bin_dir / "rfig").symlink_to(BINARY)
    env = dict(os.environ, HOME=temp, PATH=f"{bin_dir}:/bin:/usr/bin")
    subprocess.run([str(BINARY), "analyze", "rfig"], env=env, check=True, capture_output=True, text=True)
    assert (config / "generated/rfig.zsh").read_text() == script
    assert "setup\tsubcommand" in (config / "fallback/rfig.tsv").read_text()
    completion = home / "_rfig"
    completion.write_text(script)
    subprocess.run(["/bin/zsh", "-n", str(completion)], check=True)
    zsh = 'autoload -Uz compinit; compinit -D; source "$1"; compadd() { print -rl -- "$@" }; words=(rfig ""); CURRENT=2; _rfig; print -r -- SENTINEL; words=(rfig analyze ""); CURRENT=3; _rfig'
    output = subprocess.check_output(["/bin/zsh", "-fc", zsh, "rfig-test", str(completion)], env=env, text=True)
    first, second = output.split("SENTINEL\n", 1)
    assert all(name in first.splitlines() for name in ("setup", "init", "analyze", "completion")), first
    assert "git" in second.splitlines() and "mock-tool" in second.splitlines(), second
    assert "external" not in first and "pick" not in first
print("rfig public help and dynamic self-completion: OK")
