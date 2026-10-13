"""Narrow, dynamic multiline prompts through real tmux/PTY, with isolated HOME."""
import time
import unittest
from strict_shell import StrictShell, SHELL

class PromptGeometry(StrictShell):
    def test_dynamic_prompt_scroll_resize(self):
        marker='PROMPT-HEADER-'+'x'*55
        if SHELL=='bash':
            setup="_dynamic_prompt() { printf x >> \"$HOME/prompt-calls\"; printf '%s\\nTIME:%s\\nRFIG> ' '"+marker+"' \"$(date +%H:%M:%S)\"; }; PS1='$(_dynamic_prompt)'"
        elif SHELL=='zsh':
            setup="setopt promptsubst; _dynamic_prompt() { printf x >> \"$HOME/prompt-calls\"; printf '%s\\nTIME:%s\\nRFIG> ' '"+marker+"' \"$(date +%H:%M:%S)\"; }; PROMPT='$(_dynamic_prompt)'"
        else:
            setup="function fish_prompt; printf x >> $HOME/prompt-calls; printf '%s\\nTIME:%s\\nRFIG> ' '"+marker+"' (date +%H:%M:%S); end"
        script=self.home/'dynamic-prompt'; script.write_text(setup+'\n')
        self.type('source '+str(script)); self.keys('Enter')
        self.wait(lambda:(self.home/'prompt-calls').exists() and self.current()=='','dynamic prompt installed')
        self.call('resize-window','-t','test','-x','58','-y','12')
        time.sleep(.2)
        before=(self.home/'prompt-calls').read_text()
        initial_headers=self.screen().count('PROMPT-HEADER-')
        self.type('rfigprobe spa')
        self.wait(lambda:self.label_visible('space value'),'narrow menu')
        self.assertEqual(self.screen().count('PROMPT-HEADER-'),initial_headers,'editing duplicated prompt header')
        if SHELL=='bash':
            self.assertEqual((self.home/'prompt-calls').read_text(),before,'editing re-executed dynamic prompt')
        for width in (38,72,58):
            self.call('resize-window','-t','test','-x',str(width),'-y','10')
            time.sleep(.2)
            self.type('c'); self.keys('BSpace')
            self.wait(lambda:self.label_visible('space value'),'menu after resize')
        self.keys('Right','Enter')
        self.wait(lambda:len(self.executed())==1,'selected command execution')
        self.assertEqual(self.executed()[0],['space value'])
        self.wait(lambda:self.current()=='','prompt after execution')
        self.assertNotIn('⚑',self.screen())
        self.assertNotIn('→',self.screen())
        # Scroll real command output, then repeatedly enter/leave editing.
        self.type("printf 'OUTPUT\\n%.0s' {1..20}" if SHELL!='fish' else "printf 'OUTPUT\\n'; seq 20")
        self.keys('Enter'); time.sleep(.3)
        for n in range(3):
            previous=self.call('capture-pane','-p','-t','test','-S','-').count('PROMPT-HEADER-')
            self.type('rfigprobe spa'); self.wait(lambda:self.label_visible('space value'),'menu after output')
            self.assertEqual(self.call('capture-pane','-p','-t','test','-S','-').count('PROMPT-HEADER-'),previous,'editing accumulated old prompt headers')
            self.keys('Right','Enter')
            self.wait(lambda:len(self.executed())==n+2,'repeat command')
            self.wait(lambda:self.current()=='','clean prompt')
            self.assertNotIn('→',self.screen())
            self.assertLessEqual(self.call('capture-pane','-p','-t','test','-S','-').count('PROMPT-HEADER-'),previous+1,'execution duplicated prompt')
            self.assertIn('xxxxxxxxxx',self.screen(),'prompt header erased')
        print(SHELL+': narrow dynamic prompt, scrolling, resize, execution and cleanup OK')

    def test_right_aligned_prompt(self):
        if SHELL=='bash':
            setup="PS1=$'LEFT\\[\\e[s\\e[45G\\]STATUS\\[\\e[u\\]\\nRFIG> '"
        elif SHELL=='zsh':
            setup="PROMPT=$'LEFT%{\\e[s\\e[45G%}STATUS%{\\e[u%}\\nRFIG> '"
        else:
            setup="function fish_prompt; printf 'LEFT\\nRFIG> '; end; function fish_right_prompt; printf 'STATUS'; end"
        script=self.home/'right-prompt'; script.write_text(setup+'\n')
        self.type('source '+str(script)); self.keys('Enter')
        self.wait(lambda:('STATUS' in self.screen() or SHELL=='fish') and self.current()=='','right block prompt')
        self.call('resize-window','-t','test','-x','58','-y','10')
        time.sleep(.2)
        self.type('rfigprobe spa'); self.wait(lambda:self.label_visible('space value'),'right block menu')
        self.keys('Right','Enter')
        self.wait(lambda:len(self.executed())==1 and self.current().replace('STATUS','').strip()=='','right block execution')
        self.assertEqual(self.executed()[0],['space value'])
        self.assertNotIn('→',self.screen())

    @unittest.skipUnless(SHELL=='bash', 'Bash prompt expansion option')
    def test_promptvars_disabled(self):
        self.type("shopt -u promptvars; PS1='RFIG> '; touch \"$HOME/promptvars-off\""); self.keys('Enter')
        self.wait(lambda:(self.home/'promptvars-off').exists() and self.current()=='','disabled promptvars prompt')
        self.type('rfigprobe spa'); self.wait(lambda:self.label_visible('space value'),'disabled promptvars menu')
        self.keys('Right','Enter')
        self.wait(lambda:len(self.executed())==1 and self.current()=='','disabled promptvars execution')

    def test_wrapped_input_resize(self):
        self.call('resize-window','-t','test','-x','58','-y','10')
        argument='x'*95
        self.type('rfigprobe '+argument)
        time.sleep(.2)
        for width in (38,72,58):
            self.call('resize-window','-t','test','-x',str(width),'-y','10')
            time.sleep(.2)
            self.type('y'); self.keys('BSpace')
        self.keys('Enter')
        self.wait(lambda:len(self.executed())==1,'wrapped execution')
        self.assertEqual(self.executed()[0],[argument])
        self.wait(lambda:self.current()=='','prompt after wrapped execution')
        self.assertNotIn('→',self.screen())

if __name__=='__main__':
    unittest.main(defaultTest=['PromptGeometry.test_dynamic_prompt_scroll_resize','PromptGeometry.test_wrapped_input_resize','PromptGeometry.test_right_aligned_prompt','PromptGeometry.test_promptvars_disabled'])
