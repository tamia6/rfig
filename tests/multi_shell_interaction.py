"""Real keypresses in isolated tmux sessions. No user's shell config is loaded."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
BIN = Path(os.environ.get('RFIG_TEST_BINARY', ROOT / 'target/debug/rfig'))
SHELL = os.environ.get('RFIG_TEST_SHELL', 'fish')
EXE = os.environ.get('RFIG_TEST_SHELL_EXE', shutil.which(SHELL))
with tempfile.TemporaryDirectory() as temp:
    home=Path(temp)
    a=home/'a'; b=home/'b'; a.mkdir(); b.mkdir()
    config=home/'.config/rfig'; config.mkdir(parents=True)
    bins=home/'bin';bins.mkdir();(bins/'rfig').symlink_to(BIN)
    (config/'history.tsv').write_text(f'{a}\x1frfigfixture beta\n{b}\x1frfigfixture alpha\n')
    (config/'fallback').mkdir()
    (config/'fallback/rfigunknown.tsv').write_text('create\tsubcommand\n--role\toption\n-v\tflag\n')
    (bins/'rfigunknown').write_text('#!/bin/sh\nexit 0\n'); (bins/'rfigunknown').chmod(0o755)
    (config/'usage.log').write_text('C\x1frfigfixture\x1fbeta\n')
    # Current-session functions and completion definitions must work without installation.
    if SHELL=='fish':
        rc=home/'.config/fish/config.fish'; rc.parent.mkdir(parents=True,exist_ok=True)
        rc.write_text(f'''function fish_prompt; printf 'RFIG> '; end
function rfigfixture; printf '%s\\n' $argv > {home}/executed; end
complete -c rfigfixture -f -n 'not __fish_seen_subcommand_from alpha' -a 'alpha beta gamma'
complete -c rfigfixture -f -n '__fish_seen_subcommand_from alpha' -a 'north south'
complete -c rfigfixture -s v -l verbose
source {ROOT}/rfig.fish
''')
        command=[EXE,'-i']
    else:
        rc=home/'.bashrc'
        rc.write_text(f'''PS1='RFIG> '
rfigfixture() {{ printf '%s\\n' "$@" > {home}/executed; }}
_rf_fixture() {{ local choices='alpha beta gamma -v --verbose'; [[ ${{COMP_WORDS[1]}} == alpha && $COMP_CWORD -gt 1 ]] && choices='north south'; COMPREPLY=( $(compgen -W "$choices" -- "${{COMP_WORDS[COMP_CWORD]}}") ); }}
complete -F _rf_fixture rfigfixture
source {ROOT}/rfig.bash
''')
        command=[EXE,'--noprofile','--rcfile',str(rc),'-i']
    if os.environ.get('RFIG_TEST_VI'):
        text=rc.read_text()
        if SHELL=='fish':
            text=text.replace(f'source {ROOT}/rfig.fish', f'set -g fish_key_bindings fish_vi_key_bindings\nfish_vi_key_bindings\nsource {ROOT}/rfig.fish')
            text=text.replace("printf 'RFIG> '", "printf 'RFIG-TOP\\nRFIG> '")
        else:
            text=text.replace(f'source {ROOT}/rfig.bash', f'set -o vi\nsource {ROOT}/rfig.bash')
            text=text.replace("PS1='RFIG> '", "PS1=$'RFIG-TOP\\nRFIG> '")
        rc.write_text(text)
    env=dict(os.environ,HOME=temp,ZDOTDIR=temp,XDG_CONFIG_HOME=str(home/'.config'),PATH=f'{bins}:'+os.environ['PATH'],TERM='xterm-256color')
    env.pop('RFIG_PROVIDER',None)
    tmux=['tmux','-L',f'rfig-{SHELL}-{os.getpid()}','-f','/dev/null']
    def call(*args):return subprocess.check_output(tmux+list(args),env=env,text=True)
    def screen():return call('capture-pane','-t','test','-p')
    def keys(*args):call('send-keys','-t','test',*args)
    def type_(text):keys('-l',text)
    def wait_for(check, message):
        deadline=time.monotonic()+7
        while time.monotonic()<deadline:
            if check():return
            time.sleep(.05)
        raise AssertionError(message+'\n'+screen())
    try:
        call('new-session','-d','-s','test','-c',str(a),'-x','85','-y','22',*command)
        call('set-option','-g','status','off')
        wait_for(lambda:'RFIG>' in screen(),'prompt missing')
        type_('rfigfi')
        wait_for(lambda:'rfigfixture beta' in screen() and '⌘ rfigfixture' in screen(), 'typed-prefix menu and ghost missing')
        assert 'gamma' not in screen(), 'gray suggestion must not supply menu context'
        type_('xture')
        wait_for(lambda:'① beta' in screen() and 'gamma' in screen(),'menu or usage ranking missing')
        assert 'RFIG> rfigfixture beta' in screen(), screen()
        keys('Enter')
        wait_for(lambda:(home/'executed').exists(),'Enter did not execute gray suggestion')
        assert (home/'executed').read_text()=='beta\n'
        wait_for(lambda:'gamma' not in screen(),'overlay remains after execution')
        assert 'RFIG> RFIG>' not in screen(), 'prompt was duplicated'
        (home/'executed').unlink()
        type_('rfigfixture ');wait_for(lambda:'gamma' in screen(),'menu not refreshed')
        keys('Down','Right'); wait_for(lambda:'north' in screen(),'nested completion missing'); keys('C-e','Enter')
        wait_for(lambda:(home/'executed').exists(),'selected candidate not executed')
        assert (home/'executed').read_text()=='alpha\n', (home/'executed').read_text()
        type_('rfigfixture -');wait_for(lambda:'-v' in screen(),'short option missing')
        assert '--verbose' not in screen(), screen()
        type_('-');wait_for(lambda:'--verbose' in screen(),'long option missing')
        keys('C-c');wait_for(lambda:'--verbose' not in screen(),'cancel left overlay')
        type_(f'cd {b}');keys('Enter');time.sleep(.3)
        type_('rfigfixture');wait_for(lambda:'rfigfixture alpha' in screen().rsplit('RFIG>',1)[-1],'history is not directory scoped')
        keys('C-c'); time.sleep(.25)
        type_('rfigfixture g');wait_for(lambda:'gamma' in screen(),'prefix completion missing')
        keys('Right');keys('Enter');time.sleep(.2)
        assert (home/'executed').read_text()=='gamma\n'
        type_('rfigfixture 中文');keys('Left','BSpace');keys('Enter');time.sleep(.3)
        assert (home/'executed').read_text()=='文\n', (home/'executed').read_text()
        # No gray suggestion: Enter must run the actual input, not the highlighted row.
        type_('rfigfixture be'); wait_for(lambda:'beta' in screen().rsplit('RFIG>',1)[-1], 'menu missing')
        keys('Enter'); time.sleep(.2)
        assert (home/'executed').read_text()=='be\n'
        # Ctrl+E accepts gray text; it is not executed until Enter.
        type_('rfigfixture g'); wait_for(lambda:'rfigfixture gamma' in screen().rsplit('RFIG>',1)[-1], 'gray suggestion missing')
        keys('C-e'); time.sleep(.1)
        assert (home/'executed').read_text()=='be\n'
        keys('Enter');time.sleep(.2)
        assert (home/'executed').read_text()=='gamma\n'
        # Tab returns to the host shell's completion, with no implicit execution.
        type_('rfigfixture be'); time.sleep(.2); keys('Tab'); time.sleep(.3)
        assert (home/'executed').read_text()=='gamma\n'
        assert 'rfigfixture beta' in screen().rsplit('RFIG>',1)[-1], screen()
        keys('C-c');time.sleep(.2)
        type_('rfigunknown'); wait_for(lambda:'create' in screen().rsplit('RFIG>',1)[-1], 'help-cache fallback missing')
        keys('C-c'); time.sleep(.2)
        # Bottom-of-screen drawing and resize must remove the menu before output.
        call('resize-window','-t','test','-x','42','-y','12')
        type_('rfigfixture'); wait_for(lambda:'gamma' in screen().rsplit('RFIG>',1)[-1], 'menu missing after resize')
        keys('Enter');time.sleep(.2)
        assert '→' not in screen().rsplit('RFIG>',1)[-1], screen()
        wait_for(lambda:screen().rsplit('RFIG>',1)[-1].strip()=='', 'final command did not finish')
        keys('C-d'); time.sleep(.15)
        print(SHELL+' real interactive menu, gray history, ranking, options, cancellation, Unicode: OK')
    finally:
        subprocess.run(tmux+['kill-server'],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
