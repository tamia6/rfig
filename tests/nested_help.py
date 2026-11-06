"""Help-only definitions are analyzed to leaves and consumed by every shell."""
import json
import subprocess
import unittest
from strict_shell import BIN, SHELL, ARTIFACTS
from completion_output import CompletionOutput


class NestedHelp(CompletionOutput):
    def wait_menu(self, line, labels, excluded=()):
        def ready():
            rows = self.current().splitlines()
            menu = '\n'.join(rows[1:])
            return rows and rows[0].strip() == line.strip() and all(label in menu for label in labels) and not any(label in menu for label in excluded)
        self.wait(ready, 'menu for '+line)

    def test_recursive_help(self):
        for name, full in [('rfigtree', True), ('rfigother', False)]:
            tool = self.home / 'bin' / name
            tool.write_text('''#!/usr/bin/python3
import json, os, sys
name=os.path.basename(sys.argv[0]); args=sys.argv[1:]
if not args or args[-1] not in ('--help','-h'):
    with open(os.environ['RFIG_EXECUTED'],'a') as f:f.write(json.dumps(args)+'\\n')
    sys.exit(0)
path=args[:-1]
if path==['broken']:
    print('help unavailable', file=sys.stderr); sys.exit(9)
def help_for(path):
    if not path: children=['workers','levels','loop','broken']
    elif path==['workers']: children=['versions','deploy']
    elif path==['workers','versions']: children=['list','get']
    elif path[0]=='levels' and path[1:]==['n'+str(i) for i in range(len(path)-1)] and len(path)<18:children=['n'+str(len(path)-1)]
    else:children=[]
    invocation=' '.join([name]+path)
    out='Usage: '+invocation+' [command]\\n\\nCommands\\n'
    for child in children:
        label=(invocation+' ' if name=='rfigtree' else '')+child
        out+='  '+label+'  Description\\n'+' '*(len(label)+4)+'with wrapped prose\\n'
    return out+'\\nOptions:\\n  --role <role>  Role\\n  --verbose  Verbose\\n'
print(help_for([] if path==['loop'] else path))
''')
            tool.chmod(0o755)
            old = self.config/'help'/name/'subcommands/broken.tsv'
            old.parent.mkdir(parents=True, exist_ok=True)
            old.write_text('cached-child\tsubcommand\n')
            p = subprocess.run([str(BIN), 'analyze', name], env=self.env,
                               capture_output=True, text=True, timeout=30)
            ARTIFACTS.mkdir(parents=True, exist_ok=True)
            (ARTIFACTS/(name+'-analyze.log')).write_text(p.stdout+p.stderr)
            self.assertEqual(p.returncode, 0, p.stdout+p.stderr)
            self.assertEqual(old.read_text(), 'cached-child\tsubcommand\n', 'failed help must preserve previous cache')
            cache = self.config / 'help' / name
            deepest = cache / 'subcommands/levels'
            for i in range(17): deepest = deepest / f'n{i}'
            self.assertTrue(deepest.with_suffix('.tsv').exists(), 'analysis stopped before deep descendants')
            self.assertNotIn('\tsubcommand', deepest.with_suffix('.tsv').read_text(), 'deep leaf still has child commands')
            self.assertEqual((cache/'subcommands/loop.tsv').read_text(), '', 'parent-help repetition must stop')
            # Simulate a same-named or incomplete native definition returning file fallback.
            completions = self.home/'.config/fish/completions'
            completions.mkdir(parents=True, exist_ok=True)
            (completions/(name+'.fish')).write_text(
                f"complete -c {name} -f -n 'not __fish_seen_subcommand_from get' -a 'bin/ tmp/'\n"
                f"complete -c {name} -f -n '__fish_seen_subcommand_from get' -a 'dynamic-item'\n")
            if SHELL=='zsh': self.load_definition('rehash')
            self.type(name+' workers ')
            self.wait_menu(name+' workers ', ['versions','deploy'])
            self.assertNotIn('levels', self.current())
            self.keys('C-u'); self.type(name+' workers versions ')
            self.wait_menu(name+' workers versions ', ['list','get'])
            self.keys('C-u'); self.type(name+' workers --role admin versions ')
            self.wait_menu(name+' workers --role admin versions ', ['list','get'])
            self.keys('C-u'); self.type(name+' workers versions get --')
            self.wait_menu(name+' workers versions get --', ['--role','--verbose'], ['versions'])
            self.assertNotIn('versions', '\n'.join(self.current().splitlines()[1:]))
            self.keys('C-u'); self.type(name+' workers versions get dyn')
            self.wait_menu(name+' workers versions get dyn', ['dynamic-item'])
            self.keys('C-c');self.wait(lambda:self.current()=='','cancel leaf')


if __name__=='__main__':
    unittest.main(defaultTest='NestedHelp.test_recursive_help',verbosity=2)
