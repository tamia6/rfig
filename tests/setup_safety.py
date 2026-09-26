"""Setup returns quickly; enrichment runs serially in the background."""

import os
import pathlib
import subprocess
import tempfile
import time


ROOT = pathlib.Path(__file__).resolve().parents[1]

with tempfile.TemporaryDirectory() as temp:
    home = pathlib.Path(temp)
    commands = home / "bin"
    commands.mkdir()
    config = home / ".config/rfig"
    config.mkdir(parents=True)
    (config / "rfig.zsh").write_text((ROOT / "rfig.zsh").read_text())
    marker = home / "executed"
    for name in ("rfig-background-first", "rfig-background-last"):
        tool = commands / name
        tool.write_text(
            f'#!/bin/sh\nprintf "%s start\\n" "$0" >> "{marker}"\n'
            '/bin/sleep 0.2\n'
            f'printf "%s end\\n" "$0" >> "{marker}"\n'
            'printf "Commands:\\n  run  Run\\n"\n'
        )
        tool.chmod(0o755)
    (commands / "rfig-background-linked").symlink_to(commands / "rfig-background-first")
    hung = home / "hung"
    hanging_tool = commands / "rfig-background-hang"
    hanging_tool.write_text(f'#!/bin/sh\nprintf "start\\n" >> "{hung}"\n/bin/sleep 5\nprintf "end\\n" >> "{hung}"\n')
    hanging_tool.chmod(0o755)
    escaped = home / "escaped"
    detached_tool = commands / "rfig-background-detached"
    detached_tool.write_text(f'#!/bin/sh\n(/bin/sleep 1; printf "escaped\\n" >> "{escaped}") &\nprintf "Commands:\\n  run  Run\\n"\n')
    detached_tool.chmod(0o755)
    side_effect = home / "side-effect"
    writing_tool = commands / "rfig-background-writes"
    writing_tool.write_text(
        f'#!/bin/sh\nprintf "bad\\n" > "{side_effect}"\n'
        'printf "bad\\n" > relative-side-effect\n'
        'printf "Commands:\\n  run  Run\\n"\n'
    )
    writing_tool.chmod(0o755)
    for name, suffix, directory in (
        ("rfig-fish-source", ".fish", home / ".config/fish/completions"),
        ("rfig-bash-source", "", home / ".local/share/bash-completion/completions"),
        ("rfig-zsh-source", "", home / ".zsh/completions"),
    ):
        tool = commands / name
        tool.write_text(f'#!/bin/sh\nprintf "unexpected\\n" >> "{marker}"\n')
        tool.chmod(0o755)
        directory.mkdir(parents=True)
        script = f"_{name}" if suffix == "" and name == "rfig-zsh-source" else f"{name}{suffix}"
        (directory / script).write_text("# completion fixture\n")
    env = dict(os.environ, HOME=temp, SHELL="/bin/zsh", PATH=str(commands))
    started = time.monotonic()
    for _ in range(2):
        setup = subprocess.run(
            [str(ROOT / "target/debug/rfig"), "setup"],
            env=env, cwd=home, capture_output=True, text=True, timeout=5, check=True,
        )
        assert "ready" in setup.stdout
    assert time.monotonic() - started < 2, "setup must not wait for help analysis"
    names = (config / "commands.txt").read_text().splitlines()
    assert "rfig-background-last" in names
    assert "rfig-background-linked" in names, "executable symlinks must be scanned"
    supported = (config / "supported.txt").read_text().splitlines()
    assert all(name in supported for name in ("rfig-fish-source", "rfig-bash-source", "rfig-zsh-source"))
    deadline = time.monotonic() + 8
    while time.monotonic() < deadline:
        status = config / "enrich.status"
        if status.exists() and status.read_text().startswith("done\n"):
            break
        time.sleep(0.05)
    else:
        raise AssertionError("background enrichment did not finish")
    assert not marker.exists(), "help must not write files"
    assert not hung.exists(), "timed-out command must not write files"
    assert not (config / "fallback/rfig-background-hang.tsv").exists()
    time.sleep(1.1)
    assert not escaped.exists(), "help subprocesses must not survive their parent"
    assert not side_effect.exists() and not (home / "relative-side-effect").exists(), "help must not write user files"
    for name in ("rfig-background-first", "rfig-background-last", "rfig-background-linked"):
        assert "run\tsubcommand" in (config / f"fallback/{name}.tsv").read_text()
    missing_name = subprocess.run([str(ROOT / "target/debug/rfig"), "analyze"], env=env, capture_output=True, text=True)
    assert missing_name.returncode != 0, "bulk command execution must require setup's background worker"
    print("setup async serial enrichment: OK")
