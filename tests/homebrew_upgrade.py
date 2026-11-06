"""A Homebrew opt switch updates every configured shell without another setup."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
from process_cleanup import cleanup_process_tree

ROOT = Path(__file__).resolve().parents[1]
BIN = ROOT / 'target/debug/rfig'
SHELLS = ('zsh', 'bash', 'fish')

with tempfile.TemporaryDirectory(prefix='rfig-brew-upgrade-') as temp:
    root = Path(temp)
    prefix = root / 'brew with spaces'
    opt = prefix / 'opt/rfig'
    opt.parent.mkdir(parents=True)
    kegs = []
    for version in ('1', '2'):
        keg = prefix / f'Cellar/rfig/{version}'
        (keg / 'bin').mkdir(parents=True)
        (keg / 'share/rfig').mkdir(parents=True)
        shutil.copy2(BIN, keg / 'bin/rfig')
        for shell in SHELLS:
            text = f'set -gx RFIG_ADAPTER_VERSION {version}\n' if shell == 'fish' else f'export RFIG_ADAPTER_VERSION={version}\n'
            (keg / f'share/rfig/rfig.{shell}').write_text(text)
        kegs.append(keg)
    opt.symlink_to(kegs[0], target_is_directory=True)
    home = root / 'home'
    config = home / '.config/rfig'
    config.mkdir(parents=True)
    history = config / 'history.tsv'
    history.write_text('/example\x1fdo-not-change\n')
    empty = root / 'empty-path'
    empty.mkdir()
    env = dict(os.environ, HOME=str(home), ZDOTDIR=str(home),
               XDG_CONFIG_HOME=str(home / '.config'), PATH=str(empty))
    for name in ('RFIG_PROVIDER', 'SHELL'):
        env.pop(name, None)

    def setup(binary, shell):
        subprocess.run([str(binary), 'setup', '--shell', shell], env=env,
                       capture_output=True, text=True, check=True, timeout=10)
        status = config / 'enrich.status'
        deadline = time.monotonic() + 6
        while time.monotonic() < deadline:
            if status.exists() and status.read_text().startswith(('done\n', 'unavailable\n')):
                return
            time.sleep(.025)
        raise AssertionError('background setup worker did not finish')

    def sourced_version(shell):
        exe = shutil.which(shell)
        assert exe, f'{shell} required for upgrade verification'
        adapter = config / f'rfig.{shell}'
        script = 'source "$argv[1]"; printf "%s" "$RFIG_ADAPTER_VERSION"' if shell == 'fish' else 'source "$1"; printf "%s" "$RFIG_ADAPTER_VERSION"'
        args = [exe, '-c', script] + ([] if shell == 'fish' else ['rfig-test']) + [str(adapter)]
        return subprocess.check_output(args, env=env, text=True, timeout=5)

    try:
        for shell in SHELLS:
            rc = home / ({'zsh': '.zshrc', 'bash': '.bashrc', 'fish': '.config/fish/conf.d/rfig.fish'}[shell])
            rc.parent.mkdir(parents=True, exist_ok=True)
            rc.write_text(f'# Keep user settings\nsource "/old/homebrew/share/rfig/rfig.{shell}"\n')
            for _ in range(2):
                setup(opt / 'bin/rfig', shell)
            adapter = config / f'rfig.{shell}'
            assert adapter.is_symlink(), f'{shell}: setup left a stale adapter copy'
            assert Path(os.readlink(adapter)) == opt.parent.resolve() / f'rfig/share/rfig/rfig.{shell}'
            assert sourced_version(shell) == '1'
            assert rc.read_text().count('source ') == 1 and '# Keep user settings' in rc.read_text()

        opt.unlink()
        opt.symlink_to(kegs[1], target_is_directory=True)
        for shell in SHELLS:
            assert sourced_version(shell) == '2', f'{shell}: upgrade still loads old adapter'
        assert history.read_text() == '/example\x1fdo-not-change\n'

        for shell in SHELLS:
            package = kegs[1] / f'share/rfig/rfig.{shell}'
            original = package.read_bytes()
            setup(BIN, shell)
            adapter = config / f'rfig.{shell}'
            assert not adapter.is_symlink(), 'standalone setup must replace the package link'
            assert adapter.read_bytes() == (ROOT / f'rfig.{shell}').read_bytes()
            assert package.read_bytes() == original, 'setup overwrote a Homebrew-owned script'
        setup(kegs[0] / 'bin/rfig', 'zsh')
        assert not (config / 'rfig.zsh').is_symlink(), 'inactive keg must not bind a different version'
        (kegs[1] / 'share/rfig/rfig.fish').unlink()
        setup(opt / 'bin/rfig', 'fish')
        assert not (config / 'rfig.fish').is_symlink(), 'missing packaged adapter needs embedded fallback'
    finally:
        assert not cleanup_process_tree(roots=[root]), 'upgrade test processes survived cleanup'
print('Homebrew upgrade: zsh, Bash, Fish follow opt; standalone migration preserves package files')
