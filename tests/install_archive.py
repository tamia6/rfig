"""Check both install.sh paths in temporary homes."""

import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import sys
import platform
import tempfile


ROOT = Path(__file__).resolve().parents[1]

with tempfile.TemporaryDirectory() as temp:
    root = Path(temp)
    fake = root / "fake-rfig"
    fake.write_text("#!/bin/sh\nprintf '%s\\n' \"$1\" > \"$HOME/setup-called\"\n")
    fake.chmod(0o755)

    for kind in ("binary", "source"):
        home = root / kind
        home.mkdir()
        package = root / f"{kind} downloaded package" / "rfig-vtest"
        package.mkdir(parents=True)
        shutil.copy(ROOT / "install.sh", package)
        shutil.copy(ROOT / "rfig.zsh", package)
        path = "/usr/bin:/bin"

        if kind == "binary":
            shutil.copy(fake, package / "rfig")
            target = 'macos-universal' if sys.platform == 'darwin' else 'linux-' + {'arm64':'aarch64','aarch64':'aarch64','x86_64':'x86_64'}[platform.machine()]
            (package / "TARGET").write_text(target+'\n')
            (package / "SHA256SUMS").write_text(
                "".join(
                    f"{hashlib.sha256((package / name).read_bytes()).hexdigest()}  {name}\n"
                    for name in ("rfig", "rfig.zsh", "TARGET")
                )
            )
        else:
            (package / "Cargo.toml").write_text("[package]\nname='rfig'\n")
            mock_bin = root / "mock-bin"
            mock_bin.mkdir()
            cargo = mock_bin / "cargo"
            cargo.write_text(
                '#!/bin/sh\nwhile [ "$1" != "--root" ]; do shift; done\n'
                'mkdir -p "$2/bin"\ncp "$RFIG_TEST_BINARY" "$2/bin/rfig"\n'
            )
            cargo.chmod(0o755)
            path = f"{mock_bin}:{path}"

        env = dict(
            os.environ,
            HOME=str(home),
            ZDOTDIR=str(home),
            SHELL="/bin/zsh",
            PATH=path,
            RFIG_TEST_BINARY=str(fake),
        )
        if kind == "binary":
            (package / "TARGET").write_text('unsupported-other\n')
            rejected = subprocess.run(["sh", str(package / "install.sh")], env=env, capture_output=True, text=True)
            assert rejected.returncode != 0 and (package / "rfig").exists()
            assert not (home / "setup-called").exists()
            (package / "TARGET").write_text(target+'\n')
            original = (package / "rfig").read_bytes()
            (package / "rfig").write_bytes(original+b'\n# corrupted\n')
            rejected = subprocess.run(["sh", str(package / "install.sh")], env=env, capture_output=True, text=True)
            assert rejected.returncode != 0 and (package / "rfig").exists()
            assert not (home / "setup-called").exists()
            (package / "rfig").write_bytes(original)
        subprocess.run(["sh", str(package / "install.sh")], env=env, check=True)
        if kind == "binary":
            assert not (package / "rfig").exists(), "binary should be moved"
        assert (home / ".local/bin/rfig").is_file()
        assert (home / "setup-called").read_text().strip() == "setup"
        # setup owns per-shell integration; see multi_shell_setup.py with the real binary.
        assert not (home / ".zshrc").exists(), "installer must not assume zsh"

print("shared source and archive installer: OK")
