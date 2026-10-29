"""History is an explicit menu choice, never implicit input (all three shells)."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
from process_cleanup import cleanup_tmux

ROOT = Path(__file__).resolve().parents[1]
SHELL = os.environ.get('RFIG_TEST_SHELL', 'zsh')
EXE = os.environ.get('RFIG_TEST_SHELL_EXE', shutil.which(SHELL))
with tempfile.TemporaryDirectory(prefix='rfig-history-menu-') as temp:
    home = Path(temp)
    a=home/'a'; b=home/'b'; a.mkdir(); b.mkdir()
    config=home/'.config/rfig'; config.mkdir(parents=True)
    (config/'history.tsv').write_text(f'{a}\x1frfigfixture beta\n{a}\x1frfigfixture \tbad\n{a}\x1fplainfixture hello\n{a}\x1fcd {b}\n{b}\x1frfigfixture alpha\n')
    (config/'usage.log').write_text('C\x1frfigfixture\x1fbeta\n')
    bins=home/'bin'; bins.mkdir(); (bins/'rfig').symlink_to(ROOT/'target/debug/rfig')
    if SHELL=='fish':
        rc=home/'.config/fish/config.fish';rc.parent.mkdir(parents=True)
        rc.write_text(f"function fish_prompt; printf 'HIST> '; end\nfunction rfigfixture; printf '%s\\n' $argv > {home}/executed; end\nfunction plainfixture; end\ncomplete -c rfigfixture -f -a 'alpha beta gamma'\nsource {ROOT}/rfig.fish\n")
        command=[EXE,'-i']
    else:
        text=f"rfigfixture() {{ printf '%s\\n' \"$@\" > {home}/executed; }}\nplainfixture() {{ :; }}\n"
        if SHELL=='zsh':
            rc=home/'.zshrc';text="PROMPT='HIST> '\nautoload -Uz compinit; compinit -D\n"+text+"_fixture() { compadd alpha beta gamma; }; compdef _fixture rfigfixture\n";command=[EXE,'-i']
        else:
            rc=home/'.bashrc';text="PS1='HIST> '\n"+text+"_fixture() { COMPREPLY=( $(compgen -W 'alpha beta gamma' -- \"$2\") ); }; complete -F _fixture rfigfixture\n";command=[EXE,'--noprofile','--rcfile',str(rc),'-i']
        if SHELL=='zsh':
            text += '_ready() { print -r -- "$CURSOR" > "$HOME/ready"; }; zle -N _ready; bindkey "^X" _ready\n'
        rc.write_text(text+f'source {ROOT}/rfig.{SHELL}\n')
    env=dict(os.environ,HOME=temp,ZDOTDIR=temp,XDG_CONFIG_HOME=str(home/'.config'),PATH=f'{bins}:'+os.environ['PATH'],TERM='xterm-256color')
    env.pop('RFIG_PROVIDER',None)
    tm=['tmux','-L',f'history-menu-{SHELL}-{os.getpid()}','-f','/dev/null']
    def call(*args):return subprocess.check_output(tm+list(args),env=env,text=True,stderr=subprocess.DEVNULL,timeout=5)
    def keys(*args):call('send-keys','-t','test',*args)
    def type_(text):
        keys('-l',text)
        wait(lambda:bool(screen().splitlines()) and screen().splitlines()[0]==text, 'typed input without inline text')
    def screen():return call('capture-pane','-p','-t','test').rsplit('HIST>',1)[-1].removeprefix(' ')
    def wait(check,message):
        end=time.monotonic()+6
        while time.monotonic()<end:
            if check():return
            time.sleep(.025)
        raise AssertionError(message+'\n'+screen())
    result=home/'executed'
    def execute(expected):
        result.unlink(missing_ok=True);keys('Enter');wait(result.exists,'execution');assert result.read_text()==expected, result.read_text();wait(lambda:not screen().strip(),'prompt cleanup')
    try:
        call('new-session','-d','-s','test','-c',str(a),'-x','100','-y','22',*command);call('set-option','-g','status','off')
        wait(lambda:'HIST>' in call('capture-pane','-p','-t','test'),'initial prompt')
        type_('rfigfi');wait(lambda:'◷ rfigfixture beta' in screen() and '⌘ rfigfixture' in screen(),'history first with typed-prefix completions')
        assert screen().splitlines()[0]=='rfigfi',screen()
        assert 'gamma' not in screen(),screen()
        keys('C-u');type_('rfigfixture');wait(lambda:'① beta' in screen(),'ranked regular candidate')
        rows=screen().splitlines();assert '→ ◷ rfigfixture beta' in rows[1],rows
        keys('C-e');execute('\n')
        # Explicit history selection inserts the whole command, without executing.
        type_('rfigfixture be');wait(lambda:'◷ rfigfixture beta' in screen(),'history suggestion')
        execute('be\n')
        type_('rfigfixture');wait(lambda:'◷ rfigfixture be' in screen(),'latest history first')
        keys('Right');wait(lambda:screen().splitlines()[0]=='rfigfixture be','history insertion')
        execute('be\n')
        # Skip history and its first ordinary candidate, then insert alpha.
        type_('rfigfixture');wait(lambda:'① beta' in screen() and 'alpha' in screen(),'ordinary menu')
        keys('Down','Down','Right');wait(lambda:screen().splitlines()[0].strip()=='rfigfixture alpha','ordinary candidate insertion')
        execute('alpha\n')
        # History is offered even without a usable completion definition.
        type_('plainfixture h');wait(lambda:'◷ plainfixture hello' in screen(),'history-only menu')
        assert screen().splitlines()[0]=='plainfixture h',screen()
        keys('Left');keys('C-e')
        if SHELL=='zsh':
            # Wait for native completion in Ctrl+E to finish before sending SIGINT.
            keys('C-x');wait(lambda:(home/'ready').exists(), 'end-of-line widget completion')
            assert int((home/'ready').read_text()) == len('plainfixture h')
        assert screen().splitlines()[0]=='plainfixture h',screen()
        keys('C-c');wait(lambda:not screen().strip(),'cancel history')
        # Enter on cd must run cd, not the suggested directory.
        type_('cd');wait(lambda:'◷ cd ' in screen(),'cd history')
        keys('Enter');wait(lambda:Path(call('display-message','-p','-t','test','#{pane_current_path}').strip()).resolve()==home.resolve(),'cd executed typed input')
        type_(f'cd {b}');keys('Enter');wait(lambda:Path(call('display-message','-p','-t','test','#{pane_current_path}').strip()).resolve()==b.resolve(),'directory change')
        type_('rfigfixture');wait(lambda:'◷ rfigfixture alpha' in screen(),'directory-scoped history')
        assert '◷ rfigfixture beta' not in screen(),screen()
        keys('C-c')
    finally:
        assert not cleanup_tmux(tm,[home]),'test processes survived cleanup'
print(SHELL+' history-menu interaction: OK')
