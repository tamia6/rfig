from pathlib import Path
import os,subprocess,tempfile,time,sys,shlex
sys.path.insert(0,str(Path('tests').resolve()))
from process_cleanup import cleanup_tmux
with tempfile.TemporaryDirectory(prefix='rfig-homebrew-user-config-',dir='/tmp') as temp:
    root=Path(temp)
    rc='source '+shlex.quote(str(Path.home()/'.zshrc'))+'\n'
    rc += "HISTFILE=/dev/null\nPROMPT='VERIFY> '\nRPROMPT=''\n_rfig_history_dir=$PWD\n_rfig_history_lines=( 'printf rfig_homebrew_verified' )\n_rfig_history_record() { :; }\n_rfig_record_line() { :; }\n_rfig_record() { :; }\n"
    rc+='print -r -- "$(whence -p rfig)" > '+shlex.quote(str(root/'active-path'))+'\n'
    (root/'.zshrc').write_text(rc)
    env=dict(os.environ,ZDOTDIR=temp,TERM='xterm-256color')
    tm=['tmux','-L',f'rfig-brew-user-{os.getpid()}','-f','/dev/null']
    def call(*args):return subprocess.check_output(tm+list(args),env=env,text=True,stderr=subprocess.DEVNULL,timeout=5)
    def screen():return call('capture-pane','-p','-t','verify')
    def input_():return screen().rsplit('VERIFY> ',1)[-1].splitlines()[0]
    def wait(check):
        end=time.monotonic()+12
        while time.monotonic()<end:
            if check():return
            time.sleep(.03)
        raise AssertionError(screen())
    try:
        call('new-session','-d','-s','verify','-x','100','-y','24','/bin/zsh','-i')
        wait(lambda:(root/'active-path').exists() and 'VERIFY>' in screen())
        assert (root/'active-path').read_text().strip()=='/opt/homebrew/bin/rfig'
        call('send-keys','-t','verify','-l','printf rfig_home')
        wait(lambda:input_()=='printf rfig_home' and '◷ printf rfig_homebrew_verified' in screen())
        call('send-keys','-t','verify','Enter')
        wait(lambda:screen().count('VERIFY>')>=2)
        s=screen()
        assert any(line.startswith('rfig_home') for line in s.splitlines()),s
        assert 'rfig_homebrew_verified' not in s and '◷' not in s,s
        call('send-keys','-t','verify','-l','printf rfig_home')
        wait(lambda:input_()=='printf rfig_home' and '◷ printf rfig_homebrew_verified' in screen())
        call('send-keys','-t','verify','Right')
        wait(lambda:input_()=='printf rfig_homebrew_verified')
        s=screen();call('send-keys','-t','verify','C-c')
        Path('target/local-results/homebrew-upgrade-20261102/installed-user-config.screen.txt').write_text(s)
        print('Actual ~/.zshrc: Homebrew path, first-row history, typed-only Enter, explicit Right, menu cleanup: OK')
    finally:
        assert not cleanup_tmux(tm,[root]), 'user-config verification process remained'
