"""Advertised full help and invocation aliases feed every shell's menu."""
import os
import subprocess
import unittest
from strict_shell import StrictShell, SHELL, BIN

class HelpReference(StrictShell):
    def setUp(self):
        super().setUp()
        script = '''#!/bin/sh
if [ "$1" = help ] && [ "$2" = --all ]; then
  printf 'Usage:\\n  tiny setup  Configure\\n  tiny provider <sub>  Providers\\n  tiny account <sub>  Accounts\\n  tiny stop  Stop\\n\\nOptions:\\n  --help  Help\\n'
else
  printf 'Usage: tiny <command> [options]\\n\\nStart here:\\n  tiny setup  Configure\\n\\nExplore:\\n  provider account\\n\\nMore help:\\n  tiny help --all  Complete command reference\\n'
fi
'''
        for name in ('rfigreference', 'tiny'):
            tool = self.home/'bin'/name
            tool.write_text(script); tool.chmod(0o755)
            p = subprocess.run([str(BIN),'analyze',name],env=self.env,text=True,capture_output=True,timeout=10)
            self.assertEqual(p.returncode,0,p.stdout+p.stderr)
            rows = (self.config/'fallback'/f'{name}.tsv').read_text().splitlines()
            for command in ('setup','provider','account','stop'):
                self.assertIn(command+'\tsubcommand', rows)
            self.assertNotIn('Providers\tsubcommand', rows)
        if SHELL=='zsh':
            self.type('rehash'); self.keys('Enter')
            self.wait(lambda: 'RFIG> rehash' in self.screen() and self.current()=='','rehash')

    def test_full_reference_menu_and_acceptance(self):
        for name in ('rfigreference','tiny'):
            self.type(name+' pro')
            self.wait(lambda:'provider' in self.current(),'full reference menu for '+name)
            self.keys('Right')
            self.wait(lambda:name+' provider' in self.current(),'accept full reference candidate')
            self.keys('C-c')
            self.wait(lambda:self.current()=='','cancel')

    def test_failed_reference_preserves_cache(self):
        tool = self.home/'bin/rfigreference'
        cache = self.config/'fallback/rfigreference.tsv'
        before = cache.read_text()
        tool.write_text(tool.read_text().replace("then\n  printf", "then\n  exit 1\n  printf", 1))
        result = subprocess.run([str(BIN),'analyze','rfigreference'],env=self.env,text=True,capture_output=True,timeout=10)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(cache.read_text(), before)

if __name__=='__main__':
    unittest.main(defaultTest=['HelpReference.test_full_reference_menu_and_acceptance', 'HelpReference.test_failed_reference_preserves_cache'])
