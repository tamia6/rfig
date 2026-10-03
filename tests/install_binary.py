"""Install a real compiled archive from a path containing spaces, for all three shells."""
import hashlib
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tarfile
import tempfile
import time

ROOT=Path(__file__).resolve().parents[1]
BIN=Path(os.environ.get('RFIG_TEST_BINARY',ROOT/'target/release/rfig'))
with tempfile.TemporaryDirectory() as temp:
    root=Path(temp)
    for shell in ('zsh','bash','fish'):
        home=root/shell; home.mkdir()
        source=home/'package'; source.mkdir()
        shutil.copy(BIN,source/'rfig'); shutil.copy(ROOT/'install.sh',source/'install.sh')
        target='macos-universal' if platform.system()=='Darwin' else 'linux-'+platform.machine()
        (source/'TARGET').write_text(target+'\n')
        (source/'SHA256SUMS').write_text(''.join(hashlib.sha256((source/n).read_bytes()).hexdigest()+'  '+n+'\n' for n in ('rfig','TARGET')))
        archive=home/'release.tar.gz'
        with tarfile.open(archive,'w:gz') as out:out.add(source,arcname='rfig-test')
        download=home/'arbitrary download folder'; download.mkdir()
        with tarfile.open(archive) as bundle:bundle.extractall(download)
        package=download/'rfig-test'
        env=dict(os.environ,HOME=str(home),ZDOTDIR=str(home),XDG_CONFIG_HOME=str(home/'.config'),PATH='/usr/bin:/bin')
        p=subprocess.run(['sh',str(package/'install.sh'),'--shell',shell],env=env,cwd=root,capture_output=True,text=True,timeout=10)
        assert p.returncode==0,p.stdout+p.stderr
        installed=home/'.local/bin/rfig'
        assert installed.is_file() and not (package/'rfig').exists()
        assert (home/f'.config/rfig/rfig.{shell}').read_bytes()==(ROOT/f'rfig.{shell}').read_bytes()
        assert 'Usage: rfig' in subprocess.check_output([str(installed),'--help'],env=env,text=True)
        deadline=time.monotonic()+10
        status=home/'.config/rfig/enrich.status'
        while time.monotonic()<deadline:
            if status.exists() and status.read_text().startswith(('done\n','unavailable\n')):break
            time.sleep(.05)
        else:raise AssertionError('installed background worker did not finish')
        print(f'{shell}: real archive installed, executable and embedded adapter verified')
