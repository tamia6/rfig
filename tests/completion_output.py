"""Completion diagnostics stay private; explicit command execution stays visible."""
import shlex
import unittest
from strict_shell import StrictShell, SHELL


class CompletionOutput(StrictShell):
    def load_definition(self, text):
        file = self.home / 'noisy-completion'
        marker = self.home / 'definition-loaded'
        marker.unlink(missing_ok=True)
        file.write_text(text + '\n' + 'printf loaded > ' + shlex.quote(str(marker)) + '\n')
        self.type('source ' + shlex.quote(str(file)))
        self.keys('Enter')
        self.wait(lambda: marker.exists() and self.current() == '', 'completion definition loaded')

    def assert_clean_menu(self, command):
        self.type(command + ' ')
        self.wait(lambda: 'alpha' in self.current() and 'beta' in self.current(), 'valid candidates')
        self.keys('Down', 'Up')
        self.assertNotIn('RFIG_DIAGNOSTIC', self.screen())
        self.assertNotIn('RFIG_UNSTRUCTURED', self.screen())
        self.assertEqual(self.current().splitlines()[0].strip(), command)
        self.keys('Right')
        self.wait(lambda: self.current().splitlines()[0].strip() == command + ' alpha', 'insert real candidate')
        self.keys('C-c')
        self.wait(lambda: self.current() == '', 'cancel menu')

    def test_native_completion_output(self):
        probe = self.home / 'bin/noisy-provider'
        probe.write_text('#!/bin/sh\nprintf "RFIG_DIAGNOSTIC native warning\\n" >&2\nprintf "alpha\\nbeta\\n"\n')
        probe.chmod(0o755)
        quoted = shlex.quote(str(probe))
        if SHELL == 'zsh':
            definition = '_rfigprobe() { print RFIG_UNSTRUCTURED; local -a labels; labels=( "${(@f)$(command ' + quoted + ')}" ); compadd -- "${labels[@]}"; }'
        elif SHELL == 'bash':
            definition = '_rfigprobe() { printf "RFIG_UNSTRUCTURED\\n"; mapfile -t COMPREPLY < <(command ' + quoted + '); }'
        else:
            definition = 'complete -e -c rfigprobe\ncomplete -c rfigprobe -f -a "(command ' + quoted + ')"'
        self.load_definition(definition)
        self.assert_clean_menu('rfigprobe')
        # A real command's diagnostics must still reach the terminal.
        self.type(quoted)
        self.keys('Enter')
        self.wait(lambda: 'RFIG_DIAGNOSTIC native warning' in self.screen(), 'explicit execution diagnostics')

    def test_protocol_stderr_is_not_candidates(self):
        probe = self.home / 'bin/rfigprotocolnoise'
        probe.write_text('#!/bin/sh\nprintf "RFIG_DIAGNOSTIC protocol warning\\n" >&2\nprintf "alpha\\tbeta description\\nbeta\\tsecond choice\\n:0\\n"\n')
        probe.chmod(0o755)
        protocol = self.config / 'protocol'
        protocol.mkdir()
        (protocol / probe.name).write_text('cobra\n')
        if SHELL == 'zsh':
            self.load_definition('rehash')
        self.assert_clean_menu(probe.name)

    def test_generated_script_loading_output(self):
        name = 'rfiggeneratednoise'
        tool = self.home / 'bin' / name
        tool.write_text('#!/bin/sh\nexit 0\n')
        tool.chmod(0o755)
        generated = self.config / 'generated'
        generated.mkdir()
        script = 'printf "RFIG_UNSTRUCTURED startup\\n"\nprintf "RFIG_DIAGNOSTIC startup\\n" >&2\n'
        if SHELL == 'zsh':
            script += f'_noisy() {{ compadd alpha beta; }}\ncompdef _noisy {name}\n'
        elif SHELL == 'bash':
            script += f'_noisy() {{ COMPREPLY=(alpha beta); }}\ncomplete -F _noisy {name}\n'
        else:
            script += f"complete -c {name} -f -a 'alpha beta'\n"
        (generated / f'{name}.{SHELL}').write_text(script)
        if SHELL == 'zsh':
            self.load_definition('rehash')
        if SHELL == 'bash':
            for placeholder in ('_minimal', '_comp_complete_minimal'):
                self.load_definition(f'{placeholder}() {{ :; }}; complete -F {placeholder} {name}')
                self.assert_clean_menu(name)
        else:
            self.assert_clean_menu(name)


if __name__ == '__main__':
    unittest.main(defaultTest=['CompletionOutput.test_native_completion_output',
                              'CompletionOutput.test_protocol_stderr_is_not_candidates',
                              'CompletionOutput.test_generated_script_loading_output'], verbosity=2)
