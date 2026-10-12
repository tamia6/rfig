"""Replay the focused Bash/Fish checks using the isolated strict-shell harness."""
import importlib.util, os, pathlib, subprocess, time, unittest
root=pathlib.Path(__file__).resolve().parents[3]
spec=importlib.util.spec_from_file_location('strict_shell',root/'tests/strict_shell.py')
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
class Ports(m.StrictShell):
 def alive(self,pid):
  p=subprocess.run(['ps','-p',str(pid),'-o','stat='],capture_output=True,text=True)
  return p.returncode==0 and not p.stdout.strip().startswith('Z')
 def test_actual_colon(self):
  for prefix,key in [('lsof -i ',':'),('nc localhost ',':')]:
   self.type(prefix);self.wait(lambda:prefix.strip() in self.current(),'initial line');time.sleep(.2)
   begin=time.monotonic();self.type(key)
   self.wait(lambda:prefix+key in self.current(),'colon input blocked',seconds=.5)
   print(f'{m.SHELL}: {prefix+key!r} visible in {(time.monotonic()-begin)*1000:.0f}ms',flush=True)
   self.keys('C-c');self.wait(lambda:self.current()=='','cancel')
  self.assertEqual(self.executed(),[])
 def test_large_candidates(self):
  self.choices.write_text(''.join(f':service{i:04}\n' for i in range(1,5001)))
  snapshot=self.home/'snapshot';snapshot.mkdir()
  definitions = '_rfigprobe() { mapfile -t COMPREPLY < "$RFIG_CHOICES"; }\ncomplete -F _rfigprobe rfigprobe\n' if m.SHELL=='bash' else 'complete -c rfigprobe -f -a \'(cat "$RFIG_CHOICES")\'\n'
  (snapshot/'definitions').write_text(definitions)
  (snapshot/'names').write_text('')
  query_env=dict(self.env,RFIG_SHELL_EXE=m.EXE)
  result=subprocess.check_output([str(m.BIN),'query',m.SHELL,str(snapshot),'rfigprobe :'],env=query_env,text=True)
  labels=[row.split('\t')[0] for row in result.splitlines()[1:]]
  self.assertEqual(labels,[f':service{i:04}' for i in range(1,5001)])
  print(f'{m.SHELL}: all 5000 provider candidates retained',flush=True)
  self.type('rfigprobe ');time.sleep(.15)
  begin=time.monotonic();self.type(':')
  self.wait(lambda:'rfigprobe :' in self.current(),'large-list input blocked',seconds=.5)
  print(f'{m.SHELL}: 5000 candidates, colon visible in {(time.monotonic()-begin)*1000:.0f}ms',flush=True)
  self.wait(lambda:':service0001' in self.current(),'large-list inline menu')
  self.type('service500');self.wait(lambda:':service5000' in self.current(),'last candidate missing')
  self.keys('Right','Enter');self.wait(lambda:self.executed()==[[':service5000']],'last candidate acceptance')
suite=unittest.TestSuite(Ports(name) for name in ['test_actual_colon','test_large_candidates','test_slow_provider_input_latency_and_cancel'])
result=unittest.TextTestRunner(verbosity=2).run(suite)
raise SystemExit(not result.wasSuccessful())
