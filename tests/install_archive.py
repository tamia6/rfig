"""Check both install.sh paths in temporary homes."""

import hashlib
import os
from pathlib import Path
import shutil
import subprocess
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
            (package / "SHA256SUMS").write_text(
                "".join(
                    f"{hashlib.sha256((package / name).read_bytes()).hexdigest()}  {name}\n"
                    for name in ("rfig", "rfig.zsh")
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
        subprocess.run(["sh", str(package / "install.sh")], env=env, check=True)
        if kind == "binary":
            assert not (package / "rfig").exists(), "binary should be moved"
        assert (home / ".local/bin/rfig").is_file()
        assert (home / "setup-called").read_text().strip() == "setup"
        assert (home / ".config/rfig/rfig.zsh").read_bytes() == (ROOT / "rfig.zsh").read_bytes()
        assert 'export PATH="$HOME/.local/bin:$PATH"' in (home / ".zshrc").read_text()

print("shared source and archive installer: OK")
