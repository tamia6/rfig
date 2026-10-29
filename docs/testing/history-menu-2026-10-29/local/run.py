import json,os,signal,subprocess,time
from pathlib import Path
root=Path.cwd(); results=root/'target/local-results/history-menu-final-20261029'
summary=[]
def run(name,args,extra=None):
 env=dict(os.environ,**(extra or {}),RFIG_TEST_ARTIFACTS=str(results/name),RFIG_TEST_BINARY=str(root/'target/debug/rfig'));start=time.monotonic()
 p=subprocess.Popen(args,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,start_new_session=True)
 try: output,_=p.communicate(timeout=180);code=p.returncode
 except subprocess.TimeoutExpired:
  os.killpg(p.pid,signal.SIGKILL);output,_=p.communicate();code=124
 (results/(name+'.log')).write_text(output)
 entry=dict(name=name,exit_code=code,seconds=round(time.monotonic()-start,2));summary.append(entry)
 (results/'summary.json').write_text(json.dumps(summary,indent=2));print(entry,flush=True)
run('unit',['cargo','test','--locked'])
run('build',['cargo','build','--locked'])
for test in ('test_process_cleanup','multi_shell_setup','install_archive','install_binary','setup_safety','self_completion','zsh_integration','zsh_usage','zsh_array_slices','zsh_history_suggestions','zsh_large_completion','zsh_path_completion'):
 run(test,['python3',f'tests/{test}.py'])
for shell,exe in [('bash','/tmp/rfig-issue6-bash/bash-4.4/bash'),('fish','/opt/homebrew/bin/fish'),('zsh','/bin/zsh')]:
 env=dict(RFIG_TEST_SHELL=shell,RFIG_TEST_SHELL_EXE=exe)
 for test in ('history_menu','help_reference','prompt_geometry','strict_shell'):
  run(shell+'-'+test,['python3',f'tests/{test}.py'],env)
 if shell!='zsh':
  for mode in ('emacs','vi'):
   run(shell+'-'+mode,['python3','tests/multi_shell_interaction.py'],dict(env,RFIG_TEST_VI='1' if mode=='vi' else ''))
print('FAILED:',[r['name'] for r in summary if r['exit_code']])
