import os,sys,unittest,shlex
from pathlib import Path
sys.path.insert(0,str(Path('tests').resolve()))
from strict_shell import StrictShell
from completion_output import CompletionOutput
class RealPnpm(CompletionOutput):
    def test_pnpm_warning(self):
        tool=self.home/'bin/pnpm'
        tool.write_text('#!/bin/sh\nif [ -t 2 ]; then\n  printf "%s\\n" "pnpm is running through Node.js because the script that installs its native binary was skipped." "Reinstall pnpm with its build scripts allowed to run the binary directly." >&2\nfi\nexec /opt/homebrew/bin/pnpm "$@"\n')
        tool.chmod(0o755)
        adapter=os.environ.get('RFIG_REAL_ADAPTER')
        prefix=('source '+shlex.quote(adapter)+'\n') if adapter else ''
        self.load_definition(prefix+Path('/opt/homebrew/share/zsh/site-functions/_pnpm').read_text()+'\nrehash')
        self.type('pnpm ')
        self.wait(lambda:(self.current().splitlines() or [''])[0].strip()=='pnpm' and '→' in self.current() and any(word in self.current() for word in ('access','install','recursive')),'real pnpm candidates',seconds=6)
        self.assertNotIn('pnpm is running through Node.js',self.screen())
        self.assertEqual((self.current().splitlines() or [''])[0].strip(),'pnpm')
        Path(os.environ['RFIG_TEST_ARTIFACTS']).mkdir(parents=True,exist_ok=True)
        Path(os.environ['RFIG_TEST_ARTIFACTS'],'clean-menu.screen.txt').write_text(self.screen())
        self.keys('Down','Right')
        self.wait(lambda:(self.current().splitlines() or [''])[0].strip()!='pnpm','real pnpm selection')
        self.assertTrue((self.current().splitlines() or [''])[0].strip().startswith('pnpm '))
        self.keys('C-c');self.wait(lambda:self.current()=='','cancel real pnpm')
        self.type('pnpm --version');self.keys('Enter')
        self.wait(lambda:'pnpm is running through Node.js' in self.screen(),'explicit pnpm warning preserved')
unittest.main(defaultTest='RealPnpm.test_pnpm_warning',verbosity=2)
