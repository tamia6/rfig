"""Real ZLE path insertion, including prefixes hidden by native completion."""
import os
from pathlib import Path
import subprocess
import tempfile
import time
from process_cleanup import cleanup_tmux

ROOT = Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='rfig-path-') as temp:
    home = Path(temp)
    parent = home / 'parent'
    (parent / 'child dir' / 'next').mkdir(parents=True)
    (parent / 'file name.txt').write_text('fixture')
    (home / '.zshrc').write_text(
        "PROMPT='PATH> '\nautoload -Uz compinit; compinit -D\n"
        f"source {ROOT}/rfig.zsh\n"
        '_snapshot() { print -r -- "$BUFFER" > "$HOME/buffer"; print -rl -- "${_rfig_hits[@]}" > "$HOME/hits"; }\n'
        "zle -N _snapshot; bindkey '^X' _snapshot\n"
        '_affix() { compadd -U -P pre- -p hidden- -s -tail -S : -i ignored- -I -end -- value; }\n'
        'compdef _affix affixfixture\n'
        '_ignored() { compset -P "*="; compadd -- alpha; }; compdef _ignored ignoredfixture\n'
        '_pathfixture() { _arguments "--path=[path]:path:_files"; }; compdef _pathfixture pathfixture\n'
    )
    env = dict(os.environ, HOME=temp, ZDOTDIR=temp, TERM='xterm-256color')
    tmux = ['tmux', '-L', f'rfig-path-{os.getpid()}', '-f', '/dev/null']
    def call(*args):
        return subprocess.check_output(tmux + list(args), env=env, text=True, stderr=subprocess.DEVNULL, timeout=5)
    def snapshot():
        (home / 'buffer').unlink(missing_ok=True)
        call('send-keys', '-t', 'test', 'C-x')
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            if (home / 'buffer').exists() and (home / 'hits').exists():
                return (home / 'buffer').read_text().rstrip('\n'), (home / 'hits').read_text().splitlines()
            time.sleep(.025)
        raise AssertionError('snapshot timeout')
    try:
        call('new-session', '-d', '-s', 'test', '-c', temp, '-x', '150', '-y', '25', '/bin/zsh', '-i')
        deadline = time.monotonic() + 5
        while 'PATH>' not in call('capture-pane', '-p', '-t', 'test'):
            assert time.monotonic() < deadline, 'initial prompt timeout'
            time.sleep(.025)
        cases = [
            ('cd /', '/'), ('ls /', '/'),
            (f'cd {parent}/', f'{parent}/'),
            ('cd ./parent/', './parent/'), ('cd ~/parent/', '~/parent/'),
            ('cd ./parent/ch', './parent/'),
            (f'cd {parent}/child\\ dir/', f'{parent}/child\\ dir/'),
            ('ls ./parent/fi', './parent/'),
            ('pathfixture --path=./parent/ch', '--path=./parent/'),
        ]
        for typed, prefix in cases:
            call('send-keys', '-t', 'test', 'C-u')
            call('send-keys', '-t', 'test', '-l', typed)
            before, hits = snapshot()
            assert hits and hits[0], (typed, hits)
            label = hits[0].split('\t')[0]
            # Labels stay short; inserted words retain the original path context.
            assert '/' not in label.rstrip('/'), (typed, label)
            call('send-keys', '-t', 'test', 'Right')
            after, _ = snapshot()
            raw = label.rstrip('/')
            quoted = raw.replace(' ', '\\ ')
            directory = (parent / raw).is_dir() if 'child\\ dir/' not in typed else True
            if prefix == '/': directory = Path('/' + raw).is_dir()
            ending = '/' if directory else ' '
            expected = typed.split(' ', 1)[0] + ' ' + prefix + quoted + ending
            assert after == expected, (typed, after, expected)
            print(f'{typed!r} -> {after!r}', flush=True)
        for typed, expected in (
            ('affixfixture ', 'affixfixture ignored-pre-hidden-value-tail:-end '),
            ('ignoredfixture x=al', 'ignoredfixture x=alpha '),
        ):
            call('send-keys', '-t', 'test', 'C-u')
            call('send-keys', '-t', 'test', '-l', typed)
            assert snapshot()[1], typed
            call('send-keys', '-t', 'test', 'Right')
            assert snapshot()[0] == expected, (typed, snapshot()[0], expected)
    finally:
        assert not cleanup_tmux(tmux, [home]), 'test processes survived cleanup'
print('zsh path completion: OK')
