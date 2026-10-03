"""No real HOME or installed-command probing in installation checks."""
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
BIN = ROOT / 'target/debug/rfig'
with tempfile.TemporaryDirectory() as temp:
    for shell, rc in [('zsh', '.zshrc'), ('bash', '.bashrc'), ('fish', '.config/fish/conf.d/rfig.fish')]:
        home = Path(temp) / shell
        home.mkdir()
        if shell == 'zsh':
            (home / rc).write_text('source "/old/homebrew/share/rfig/rfig.zsh"\n')
        env = dict(os.environ, HOME=str(home), SHELL='/bin/unsupported', PATH='/usr/bin:/bin', ZDOTDIR=str(home), XDG_CONFIG_HOME=str(home / '.config'))
        for _ in range(2):
            p = subprocess.run([str(BIN), 'setup', '--shell', shell], env=env, capture_output=True, text=True, timeout=8)
            assert p.returncode == 0, (shell, p.stderr)
        content = (home / rc).read_text()
        assert content.count('source ') == 1, content
        assert (home / f'.config/rfig/rfig.{shell}').read_bytes() == (ROOT / f'rfig.{shell}').read_bytes()
        generated = subprocess.check_output([str(BIN), 'completion', shell], env=env, text=True)
        assert 'setup' in generated and 'analyze' in generated
    p = subprocess.run([str(BIN), 'setup', '--shell', 'invalid'], env=env, capture_output=True)
    assert p.returncode != 0
print('multi-shell isolated setup: OK')
