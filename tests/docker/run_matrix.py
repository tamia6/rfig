"""Run inside /work in the disposable container as the unprivileged tester user."""
import json
import os
from pathlib import Path
import subprocess
import time

root=Path(__file__).resolve().parents[2]
results=root/'results';results.mkdir(exist_ok=True)
summary=[]
def run(name,args,extra=None,timeout=150):
    env=dict(os.environ,**(extra or {}));env['RFIG_TEST_ARTIFACTS']=str(results/name)
    start=time.monotonic()
    try:
        p=subprocess.run(args,cwd=root,env=env,capture_output=True,text=True,timeout=timeout)
        code=p.returncode;output=p.stdout+p.stderr
    except subprocess.TimeoutExpired as e:
        code=124;output='TIMEOUT\n'+str(e.stdout)+str(e.stderr)
    (results/(name+'.log')).write_text(output)
    entry=dict(name=name,exit_code=code,seconds=round(time.monotonic()-start,2))
    summary.append(entry);print(json.dumps(entry),flush=True)
    (results/'summary.json').write_text(json.dumps(summary,indent=2))

run('unit',['cargo','test','--locked','--offline','-j','2'])
run('setup',['python3','tests/multi_shell_setup.py'])
run('archive-installer',['python3','tests/install_archive.py'])
run('real-binary-installer',['python3','tests/install_binary.py'],dict(RFIG_TEST_BINARY=str(root/'target/debug/rfig')))
run('linux-probe-safety',['python3','tests/docker/linux_probe_safety.py'])
for test in ('setup_safety', 'self_completion', 'zsh_integration', 'zsh_usage', 'zsh_history_suggestions'):
    run(test,['python3',f'tests/{test}.py'])
# Exercise the real platform sandbox; no fake sandbox helpers.
for shell,exe,tag in [('bash','/bin/bash','bash-system'),('bash','/opt/bash44/bin/bash','bash44'),('fish','/usr/bin/fish','fish'),('zsh','/usr/bin/zsh','zsh59')]:
    if not Path(exe).exists():continue
    env=dict(RFIG_TEST_SHELL=shell,RFIG_TEST_SHELL_EXE=exe)
    if shell!='zsh':
        for mode in ('emacs','vi'):
            for attempt in range(1,4):
                run(f'{tag}-{mode}-{attempt}',['python3','tests/multi_shell_interaction.py'],dict(env,RFIG_TEST_VI='1' if mode=='vi' else ''))
    run(f'{tag}-strict',['python3','tests/strict_shell.py'],env)
    run(f'{tag}-ansi',['python3','tests/strict_shell.py','StrictShell.test_literal_candidates','StrictShell.test_rapid_input_delete_and_wrapping'],dict(env,RFIG_TEST_TERM='xterm'))
raise SystemExit(1 if any(item['exit_code'] for item in summary) else 0)
