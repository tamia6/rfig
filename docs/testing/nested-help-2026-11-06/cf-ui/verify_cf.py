"""Docker UI proof using host-generated cf help cache; cf execution is disabled."""
from pathlib import Path
import os, sys, shutil, subprocess, unittest, json
sys.path.insert(0,str(Path.cwd()/'tests'))
from nested_help import NestedHelp
from strict_shell import BIN, SHELL, ARTIFACTS
SOURCE=Path(__file__).resolve().parent/'cache/cf'

class CfMenus(NestedHelp):
    def selected(self):
        for row in self.current().splitlines()[1:]:
            if row.lstrip().startswith(('→','➜')):
                return row.split()[-1]
        return ''

    def choose(self, label):
        for _ in range(35):
            before=self.selected()
            if before==label:
                self.keys('Right');return
            self.keys('Down')
            self.wait(lambda:self.selected()!=before, 'selection moves')
        self.fail('missing choice '+label+'\n'+self.screen())

    def test_workers_menus(self):
        target=self.config/'help/cf';shutil.copytree(SOURCE,target)
        tool=self.home/'bin/cf'
        tool.write_text('#!/bin/sh\nprintf "%s\\n" "$*" >> "$RFIG_EXECUTED"\nexit 1\n')
        tool.chmod(0o755)
        self.load_definition('rehash' if SHELL=='zsh' else ('true' if SHELL=='fish' else ':'))
        # Audit every available workers context using the final cache reader.
        nodes=[SOURCE/'subcommands/workers.tsv']+sorted((SOURCE/'subcommands/workers').rglob('*.tsv'))
        for file in nodes:
            args=file.relative_to(SOURCE/'subcommands').with_suffix('').parts
            line='cf '+' '.join(args)+' '
            actual=subprocess.check_output([str(BIN),'external','cf',line],env=self.env,text=True,timeout=4)
            self.assertEqual(actual, 'cache\n'+file.read_text(),line)
        self.call('resize-window','-t','test','-x','58','-y','18')
        self.type('cf workers ')
        self.wait_menu('cf workers ', ['check','delete'])
        self.assertNotIn('bin/',self.current())
        ARTIFACTS.mkdir(parents=True,exist_ok=True)
        (ARTIFACTS/'workers.screen.txt').write_text(self.screen())
        self.choose('versions')
        self.wait_menu('cf workers versions ', ['create','delete','get','list'])
        (ARTIFACTS/'versions.screen.txt').write_text(self.screen())
        self.choose('list')
        self.wait(lambda:self.current().startswith('cf workers versions list'),'list inserted')
        self.keys('-l','--','--')
        self.wait_menu('cf workers versions list --',['--dry-run','--help','--local'])
        (ARTIFACTS/'list-options.screen.txt').write_text(self.screen())
        self.choose('--page')
        self.wait(lambda:self.current().startswith('cf workers versions list --page'),'option inserted')
        self.keys('C-c');self.wait(lambda:self.current()=='','menu cleaned on cancel')
        for file in nodes:
            args=file.relative_to(SOURCE/'subcommands').with_suffix('').parts
            records=[row.split('\t') for row in file.read_text().splitlines()]
            children=[label for label,kind in records if kind=='subcommand']
            labels=children or [label for label,kind in records if label.startswith('--')]
            line='cf '+' '.join(args)+' '+('' if children else '--')
            self.type(line)
            self.wait_menu(line,labels[:3])
            (ARTIFACTS/('sweep-'+'-'.join(args)+'.screen.txt')).write_text(self.screen())
            self.wait(lambda:self.selected() in labels, 'selectable cached row')
            chosen=self.selected()
            self.keys('Right')
            expected=(line if children else line[:-2])+chosen
            self.wait(lambda:(self.current().splitlines() or [''])[0].strip()==expected.strip(), 'context choice inserted')
            self.keys('C-c');self.wait(lambda:self.current()=='','context cleanup')
        self.assertFalse(self.result.exists(),'menu browsing executed cf')
        (ARTIFACTS/'summary.json').write_text(json.dumps({'shell':SHELL,'cached_workers_contexts':len(nodes),'ui_swept_contexts':len(nodes),'ui_selected_contexts':len(nodes),'width':58,'selected':['versions','list','--page'],'command_executed':False},indent=2)+'\n')

unittest.main(defaultTest='CfMenus.test_workers_menus',verbosity=2)
