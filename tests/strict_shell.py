"""Adversarial real-shell tests. Run only with an isolated HOME (created per test).

RFIG_TEST_SHELL=zsh|bash|fish RFIG_TEST_SHELL_EXE=/path/to/shell python3 tests/strict_shell.py
Failures retain a text screen in RFIG_TEST_ARTIFACTS. No real commands are scanned.
"""
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[1]
SHELL = os.environ.get('RFIG_TEST_SHELL', 'bash')
EXE = os.environ.get('RFIG_TEST_SHELL_EXE', shutil.which(SHELL))
BIN = Path(os.environ.get('RFIG_TEST_BINARY', ROOT/'target/debug/rfig'))
ARTIFACTS = Path(os.environ.get('RFIG_TEST_ARTIFACTS', '/tmp/rfig-strict-results'))

class StrictShell(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='rfig-strict-')
        self.home = Path(self.temp.name)
        self.tmp = self.home/'tmp'; self.tmp.mkdir()
        self.config=self.home/'.config/rfig'; self.config.mkdir(parents=True)
        bins=self.home/'bin'; bins.mkdir()
        (bins/'rfig').symlink_to(BIN)
        probe=bins/'rfigprobe'
        probe.write_text('#!/usr/bin/python3\nimport json,os,sys\nwith open(os.environ["RFIG_EXECUTED"],"a") as f: f.write(json.dumps(sys.argv[1:],ensure_ascii=False)+"\\n")\n')
        probe.chmod(0o755)
        (bins/'rfigslow').symlink_to(probe)
        self.result=self.home/'executed'
        self.choices=self.home/'choices'; self.choices.write_text('space value\nsemi;colon\ndollar$(touch PWN)\n中文 分支\n')
        self.marker=self.home/'slow-pids'
        slow=self.home/'slow-provider'
        slow.write_text('#!/usr/bin/python3\nimport json,os,subprocess,time\np=subprocess.Popen(["sleep","5"])\nwith open(os.environ["RFIG_SLOW_PIDS"],"a") as f:f.write(json.dumps([os.getpid(),p.pid])+"\\n")\ntime.sleep(5)\nprint("stale_choice")\n')
        slow.chmod(0o755)
        self.env=dict(os.environ, HOME=str(self.home), ZDOTDIR=str(self.home), XDG_CONFIG_HOME=str(self.home/'.config'),
            TMPDIR=str(self.tmp), PATH=f'{bins}:'+os.environ['PATH'], TERM=os.environ.get('RFIG_TEST_TERM','xterm-256color'),
            RFIG_EXECUTED=str(self.result), RFIG_CHOICES=str(self.choices), RFIG_SLOW=str(slow), RFIG_SLOW_PIDS=str(self.marker))
        self.env.pop('RFIG_PROVIDER',None)
        if SHELL=='bash':
            rc=self.home/'.bashrc'
            rc.write_text(f'''PS1='RFIG> '
_rfigprobe() {{ local label; COMPREPLY=(); while IFS= read -r label; do [[ $label == "$2"* ]] && COMPREPLY+=( "$label" ); done < "$RFIG_CHOICES"; }}
_rfigslow() {{ mapfile -t COMPREPLY < <(command "$RFIG_SLOW"); }}
complete -F _rfigprobe rfigprobe
complete -F _rfigslow rfigslow
source {ROOT}/rfig.bash
''')
            command=[EXE,'--noprofile','--rcfile',str(rc),'-i']
        elif SHELL=='fish':
            rc=self.home/'.config/fish/config.fish'; rc.parent.mkdir(parents=True)
            rc.write_text(f'''set -g fish_greeting
function fish_prompt; printf 'RFIG> '; end
complete -c rfigprobe -f -a '(cat "$RFIG_CHOICES")'
complete -c rfigslow -f -a '(command "$RFIG_SLOW")'
source {ROOT}/rfig.fish
''')
            command=[EXE,'-i']
        else:
            rc=self.home/'.zshrc'
            rc.write_text(f'''PROMPT='RFIG> '
autoload -Uz compinit; compinit -D
_rfigprobe() {{ local -a labels; labels=( "${{(@f)$(<"$RFIG_CHOICES")}}" ); compadd -- "${{labels[@]}}"; }}
_rfigslow() {{ local -a labels; labels=( "${{(@f)$(command "$RFIG_SLOW")}}" ); compadd -- "${{labels[@]}}"; }}
compdef _rfigprobe rfigprobe
compdef _rfigslow rfigslow
source {ROOT}/rfig.zsh
''')
            command=[EXE,'-i']
        self.tmux=['tmux','-L',f'strict-{SHELL}-{os.getpid()}','-f','/dev/null']
        self.call('new-session','-d','-s','test','-c',str(self.home),'-x','80','-y','18',*command)
        self.call('set-option','-g','status','off')
        self.shell_pid=int(self.call('display-message','-p','-t','test','#{pane_pid}'))
        self.wait(lambda:'RFIG>' in self.screen(), 'initial prompt')

    def call(self,*args):
        return subprocess.check_output(self.tmux+list(args),env=self.env,text=True,stderr=subprocess.DEVNULL,timeout=5)
    def screen(self):
        try:return self.call('capture-pane','-p','-t','test')
        except subprocess.CalledProcessError:return '<session exited>'
    def label_visible(self, text):
        screen=self.current()
        return text in screen or ''.join('\\'+c if c in ' ;$()' else c for c in text) in screen
    def current(self):return self.screen().rsplit('RFIG>',1)[-1].strip()
    def keys(self,*args):self.call('send-keys','-t','test',*args)
    def type(self,text):self.keys('-l',text)
    def wait(self,predicate,message,seconds=3):
        start=time.monotonic()
        while time.monotonic()-start<seconds:
            if predicate():return time.monotonic()-start
            time.sleep(.025)
        self.fail(message+'\n'+self.screen())
    def executed(self):
        try:return [json.loads(l) for l in self.result.read_text().splitlines()]
        except (FileNotFoundError,json.JSONDecodeError):return []
    def run_line(self,text,count):
        self.type(text); self.keys('Enter')
        self.wait(lambda:len(self.executed())>=count,'command execution')
        self.wait(lambda:self.current()=='','return to prompt')
    def editor_pids(self):
        out=subprocess.check_output(['ps','-eo','pid=,args='],text=True)
        return [int(l.split()[0]) for l in out.splitlines() if 'rfig edit ' in l and str(self.tmp) in l]
    def slow_pids(self):
        try:return [p for l in self.marker.read_text().splitlines() for p in json.loads(l)]
        except (FileNotFoundError,json.JSONDecodeError):return []
    def alive(self,pid):
        try:
            status=Path(f'/proc/{pid}/stat').read_text().split(') ',1)[1].split()[0]
            return status!='Z'
        except FileNotFoundError:return False
    def tearDown(self):
        ARTIFACTS.mkdir(parents=True,exist_ok=True)
        (ARTIFACTS/(self.id().split('.')[-1]+'.screen.txt')).write_text(self.screen())
        subprocess.run(self.tmux+['kill-server'],env=self.env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        for pid in self.slow_pids():
            if self.alive(pid):
                try:os.kill(pid,signal.SIGKILL)
                except ProcessLookupError:pass
        deadline=time.monotonic()+3
        while self.alive(self.shell_pid) and time.monotonic()<deadline: time.sleep(.025)
        # Fish may finish writing its generated completion cache after shell exit.
        for attempt in range(30):
            try:
                self.temp.cleanup(); break
            except OSError:
                if attempt==29: raise
                time.sleep(.05)

    def test_literal_candidates(self):
        for n,(prefix,expected) in enumerate([('spa','space value'),('semi','semi;colon'),('dollar','dollar$(touch PWN)'),('中文','中文 分支')],1):
            self.type('rfigprobe '+prefix)
            self.wait(lambda:self.label_visible(expected),'literal candidate '+expected)
            self.keys('Right'); time.sleep(.08); self.keys('Enter')
            self.wait(lambda:len(self.executed())==n,'literal candidate execution')
            self.assertEqual(self.executed()[-1],[expected])
            self.wait(lambda:self.current()=='','prompt after literal')
        self.assertFalse((self.home/'PWN').exists(),'candidate was evaluated as shell code')

    def test_rapid_input_delete_and_wrapping(self):
        self.type('rfigprobe '+'x'*220)
        self.keys('-N','100','BSpace')
        self.keys('Enter')
        self.wait(lambda:len(self.executed())==1,'rapid input execution')
        self.assertEqual(self.executed(),[['x'*120]])
        self.wait(lambda:self.current()=='','overlay cleanup after wrapped input')
        self.assertNotIn('space value',self.screen())

    def test_multiline_bracketed_paste(self):
        self.type('rfigprobe ')
        self.wait(lambda:self.label_visible('space value'),'menu before paste')
        self.call('set-buffer','--','one\nrfigprobe two')
        self.call('paste-buffer','-p','-t','test')
        time.sleep(.25)
        self.assertEqual(self.executed(),[], 'paste executed without Enter')
        self.keys('Enter')
        self.wait(lambda:len(self.executed())==2,'multiline execution')
        self.assertEqual(self.executed(),[['one'],['two']])

    def test_shell_state_and_directory(self):
        target=self.home/'space dir';target.mkdir()
        self.type('cd '+"'"+str(target)+"'");self.keys('Enter')
        self.wait(lambda:os.readlink(f'/proc/{self.shell_pid}/cwd')==str(target) and self.current()=='' and not self.editor_pids(),'cd prompt')
        self.run_line('rfigprobe "$PWD"',1)
        self.assertEqual(self.executed()[-1],[str(target)])
        assignment="set -gx RFIG_KEEP 'persistent value'" if SHELL=='fish' else "export RFIG_KEEP='persistent value'"
        self.run_line(assignment+'; rfigprobe "$RFIG_KEEP"',2)
        self.assertEqual(self.executed()[-1],['persistent value'])

    def test_cancel_repeated_and_private_history(self):
        for _ in range(10):
            self.type('rfigprobe spa')
            self.wait(lambda:self.label_visible('space value'),'menu before cancel')
            self.keys('C-c')
            self.wait(lambda:self.current()=='' and not self.editor_pids(),'cancel cleanup')
        self.assertEqual(self.executed(),[])
        self.wait(lambda:not list(self.tmp.glob('rfig.*')),'temporary snapshots leaked')
        self.run_line('rfigprobe private',1)
        history=self.config/'history.tsv'
        self.wait(history.exists,'history not saved')
        self.assertEqual(history.stat().st_mode & 0o777,0o600)

    def test_slow_provider_input_latency_and_cancel(self):
        self.type('rfigslow ')
        self.wait(lambda:self.slow_pids(),'slow provider did not start')
        started=time.monotonic(); self.type('xyz')
        latency=self.wait(lambda:'rfigslow xyz' in self.current(),'typing blocked by slow provider',seconds=.5)
        self.keys('C-c')
        self.wait(lambda:self.current()=='','cancel slow provider',seconds=.75)
        self.wait(lambda:not any(self.alive(p) for p in self.slow_pids()),'completion descendants leaked',seconds=.75)
        print(f'{SHELL}: slow-provider input observation {1000*(time.monotonic()-started):.0f}ms (screen wait {1000*latency:.0f}ms)',flush=True)
        self.assertNotIn('stale_choice',self.screen())

    def test_ctrl_d_exits_empty_line(self):
        self.type('rfigprobe spa');self.wait(lambda:self.label_visible('space value'),'menu before EOF')
        self.keys('C-u','C-d')
        self.wait(lambda:self.screen()=='<session exited>','Ctrl+D did not exit on empty line')

    def test_editor_sigterm_cleanup(self):
        if SHELL=='zsh':self.skipTest('zsh uses ZLE rather than a separate editor process')
        self.type('rfigslow ');self.wait(lambda:self.slow_pids(),'provider start')
        pids=self.editor_pids();self.assertEqual(len(pids),1)
        os.kill(pids[0],signal.SIGTERM)
        self.wait(lambda:not any(self.alive(p) for p in self.slow_pids()),'SIGTERM left completion processes running',seconds=.75)
        self.keys('C-u');self.type('rfigprobe recovered');self.keys('Enter')
        self.wait(lambda:self.executed()==[['recovered']],'terminal did not recover after SIGTERM')

if __name__=='__main__':
    print(f'SHELL={SHELL} EXE={EXE}',flush=True)
    unittest.main(verbosity=2)
