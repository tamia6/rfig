"""Port completion latency and candidate integrity; python3 tests/zsh_large_completion.py."""
import os
from pathlib import Path
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]

with tempfile.TemporaryDirectory(prefix='rfig-ports-') as temp:
    home = Path(temp)
    (home / '.zshrc').write_text(f'''
PROMPT='RFIG> '
autoload -Uz compinit; compinit -D
zmodload zsh/datetime
source {ROOT}/rfig.zsh
# Use native command definitions with a repeatable large port list.
ports=( service{{0001..5000}} )
zstyle ':completion:*' ports "${{ports[@]}}" service0001
_telnet_args=( '-l+[specify user]:user:->users' )
functions[_rfig_preview_original]=$functions[_rfig_preview]
_rfig_preview() {{
  local started=$EPOCHREALTIME
  _rfig_preview_original "$@"
  typeset -g elapsed=$(( EPOCHREALTIME - started ))
}}
_snapshot() {{
  print -rl -- "$BUFFER" "$elapsed" "${{_rfig_hits[@]}}" done > "$HOME/timing"
}}
zle -N _snapshot
bindkey '^X' _snapshot
''')
    env = dict(os.environ, HOME=temp, ZDOTDIR=temp, TERM='xterm-256color')
    tmux = ['tmux', '-L', f'rfig-ports-{os.getpid()}', '-f', '/dev/null']

    def call(*args):
        return subprocess.check_output(tmux + list(args), env=env, text=True)

    def snapshot(keys, expected):
        (home / 'timing').unlink(missing_ok=True)
        call('send-keys', '-t', 'test', '-l', keys)
        call('send-keys', '-t', 'test', 'C-x')
        deadline = time.monotonic() + 10
        while True:
            rows = (home / 'timing').read_text().splitlines() if (home / 'timing').exists() else []
            if rows and rows[-1] == 'done':
                break
            assert time.monotonic() < deadline, 'completion blocked the snapshot widget'
            time.sleep(.01)
        assert rows[0] == expected, rows[0]
        return float(rows[1]), rows[2:-1]

    try:
        call('new-session', '-d', '-s', 'test', '-x', '100', '-y', '24', '/bin/zsh', '-i')
        deadline = time.monotonic() + 5
        while 'RFIG>' not in call('capture-pane', '-t', 'test', '-p'):
            assert time.monotonic() < deadline, 'initial prompt missing'
            time.sleep(.02)
        for prefix, key in [('lsof -i ', ':'), ('telnet localhost', ' ')]:
            snapshot(prefix, prefix)
            elapsed, hits = snapshot(key, prefix + key)
            labels = [row.split('\t')[0] for row in hits]
            assert labels == [f'service{i:04}' for i in range(1, 5001)], 'candidates lost, reordered or duplicated'
            assert all(row.endswith('\targument') for row in hits), 'port classification changed'
            assert elapsed < .5, f'{prefix + key}: {elapsed:.3f}s for 5,000 ports'
            print(f'{prefix + key}: {elapsed * 1000:.0f}ms; all 5,000 unique ports retained')
            assert 'service0001' in call('capture-pane', '-t', 'test', '-p'), 'inline menu missing'
            call('send-keys', '-t', 'test', 'C-u')
    finally:
        subprocess.run(tmux + ['kill-server'], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
