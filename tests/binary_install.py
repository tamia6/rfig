"""Check the downloaded-file installation path without touching the real home."""

import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[1]

with tempfile.TemporaryDirectory() as temp:
    home = Path(temp)
    downloads = home / "Downloads"
    downloads.mkdir()
    installer_dir = home / "installer"
    installer_dir.mkdir()
    shutil.copy(ROOT / "install-binary.sh", installer_dir)
    shutil.copy(ROOT / "rfig.zsh", installer_dir)
    binary = downloads / "rfig"
    binary.write_text("#!/bin/sh\nprintf '%s\\n' \"$1\" > \"$HOME/setup-called\"\n")
    binary.chmod(0o755)
    checksums = {
        "rfig-macos-universal": binary,
        "rfig.zsh": installer_dir / "rfig.zsh",
    }
    (installer_dir / "SHA256SUMS").write_text(
        "".join(
            f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {name}\n"
            for name, path in checksums.items()
        )
    )
    env = dict(os.environ, HOME=temp, ZDOTDIR=temp, SHELL="/bin/zsh", PATH="/usr/bin:/bin")
    subprocess.run(["sh", str(installer_dir / "install-binary.sh")], env=env, check=True)
    assert not binary.exists(), "downloaded binary should be moved"
    assert (home / ".local/bin/rfig").is_file()
    assert (home / "setup-called").read_text().strip() == "setup"
    assert (home / ".config/rfig/rfig.zsh").read_bytes() == (ROOT / "rfig.zsh").read_bytes()
    assert 'export PATH="$HOME/.local/bin:$PATH"' in (home / ".zshrc").read_text()
    print("binary installer moves Downloads asset and runs setup: OK")
