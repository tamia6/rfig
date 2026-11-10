"""Exercise the shared editor through PowerShell, using ConPTY on Windows or tmux on Unix."""
import json
import os
from pathlib import Path
import queue
import shutil
import subprocess
import tempfile
import threading
import time

ROOT = Path(__file__).resolve().parents[1]
WINDOWS = os.name == 'nt'
BIN = Path(os.environ.get('RFIG_TEST_BINARY', ROOT / ('target/debug/rfig.exe' if WINDOWS else 'target/debug/rfig')))
PWSH = os.environ.get('RFIG_TEST_PWSH', shutil.which('pwsh'))
assert PWSH, 'PowerShell 7.2+ is required'

def ps_quote(value):
    return "'" + str(value).replace("'", "''") + "'"

with tempfile.TemporaryDirectory(prefix='rfig-powershell-') as temp:
    home = Path(temp)
    bin_dir = home / 'bin'
    bin_dir.mkdir()
    shutil.copy2(BIN, bin_dir / BIN.name)
    config = home / '.config/rfig'
    config.mkdir(parents=True)
    profile = home / 'profile.ps1'
    env = dict(os.environ, HOME=temp, USERPROFILE=temp, XDG_CONFIG_HOME=str(home / '.config'),
               RFIG_POWERSHELL_PROFILE=str(profile),
               PATH=str(bin_dir) + os.pathsep + os.environ['PATH'])
    # Setup is repeatable and does not touch the actual user's profile.
    for _ in range(2):
        subprocess.run([str(BIN), 'setup', '--shell', 'powershell'], env=env, check=True, capture_output=True, timeout=20)
    text = profile.read_text(encoding='utf-8')
    assert text.count(". '" + str(config / 'rfig.ps1').replace("'", "''") + "'") == 1
    assert (config / 'rfig.ps1').read_bytes() == (ROOT / 'rfig.ps1').read_bytes()
    output = home / 'executed.json'
    script = home / 'init.ps1'
    script.write_text(f"""function prompt {{ 'PS-RFIG> ' }}
function rfigfixture {{ param([string]$choice) [IO.File]::WriteAllText({ps_quote(output)}, (ConvertTo-Json -InputObject $choice -Compress)) }}
Register-ArgumentCompleter -CommandName rfigfixture -ParameterName choice -ScriptBlock {{
    param($commandName, $parameterName, $wordToComplete)
    @('alpha', 'beta', '中文', 'two words') | Where-Object {{ $_.StartsWith($wordToComplete) }} | ForEach-Object {{
        $insert = if ($_ -eq 'two words') {{ "'two words'" }} else {{ $_ }}
        [System.Management.Automation.CompletionResult]::new($insert, $_, 'ParameterValue', $_)
    }}
}}
. {ps_quote(config / 'rfig.ps1')}
""", encoding='utf-8')
    command = [PWSH, '-NoLogo', '-NoProfile', '-NoExit', '-File', str(script)]
    if WINDOWS:
        from winpty import PtyProcess
        process = PtyProcess.spawn(command, cwd=temp, env=env, dimensions=(25, 120))
        chunks = queue.Queue()
        def reader():
            try:
                while process.isalive():
                    chunks.put(process.read(65536))
            except EOFError:
                pass
        threading.Thread(target=reader, daemon=True).start()
        transcript = ['']
        def screen():
            while True:
                try: transcript[0] += chunks.get_nowait()
                except queue.Empty: return transcript[0]
        def send(text): process.write(text)
        def close(): process.close(force=True)
    else:
        tmux = ['tmux', '-L', f'rfig-ps-{os.getpid()}', '-f', '/dev/null']
        def call(*args): return subprocess.check_output(tmux + list(args), env=env, text=True)
        call('new-session', '-d', '-s', 'test', '-c', temp, '-x', '120', '-y', '25', *command)
        def screen(): return call('capture-pane', '-p', '-t', 'test')
        def send(text): call('send-keys', '-t', 'test', '-l', text)
        def close():
            import sys
            sys.path.insert(0, str(ROOT / 'tests'))
            from process_cleanup import cleanup_tmux
            assert not cleanup_tmux(tmux, [home]), 'PowerShell/editor processes survived cleanup'
    def wait_for(check, message):
        deadline = time.monotonic() + 12
        while time.monotonic() < deadline:
            if check(): return
            time.sleep(.025)
        raise AssertionError(message + '\n' + screen())
    def executed(value):
        wait_for(lambda: output.exists() and json.loads(output.read_text(encoding='utf-8-sig')) == value, f'expected execution: {value!r}')
        output.unlink()
        time.sleep(.15)
    try:
        wait_for(lambda: 'PS-RFIG>' in screen(), 'prompt missing')
        send('rfigfixture -choice ')
        wait_for(lambda: 'alpha' in screen() and 'beta' in screen(), 'registered completion missing')
        send('\x1b[1;3B')
        time.sleep(.15)
        send('\t\r')
        executed('beta')
        if not WINDOWS:
            # Traditional ESC + arrow encoding is accepted as well.
            send('rfigfixture -choice a')
            wait_for(lambda: 'alpha' in screen(), 'prefix completion missing')
            send('\x1b\x1b[B')
            time.sleep(.1)
            send('\t\r')
            executed('alpha')
        send('rfigfixture -choice two')
        wait_for(lambda: 'two words' in screen(), 'quoted completion missing')
        send('\t\r')
        executed('two words')
        send('rfigfixture -choice 中文\r')
        executed('中文')
        send('rfigfixture -choice cancelled\x03')
        time.sleep(.2)
        assert not output.exists(), 'Ctrl+C executed input'
        send('rfigfixture -choice after_cancel\r')
        executed('after_cancel')
        print('PowerShell native completion, Alt arrows, quoting, Unicode, execution and cancellation: OK')
    finally:
        close()
