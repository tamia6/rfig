import subprocess
import sys
import time
import unittest
import os
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from process_cleanup import cleanup_process_tree, cleanup_tmux, _processes


class ProcessCleanupTests(unittest.TestCase):
    def test_tmux_captures_detached_children_before_parent_exit(self):
        with tempfile.TemporaryDirectory(prefix='rfig-cleanup-home-') as temp, tempfile.TemporaryDirectory(prefix='rfig-writer-') as code:
            home = Path(temp)
            writer = Path(code)/'writer.py'
            writer.write_text("import os,time\nfrom pathlib import Path\nhome=Path(os.environ['RFIG_CLEANUP_HOME'])\n(home/'pid').write_text(str(os.getpid()))\nwhile True:\n (home/'cache').write_text('data');time.sleep(.02)\n")
            tm = ['tmux','-L',f'cleanup-detached-{os.getpid()}','-f','/dev/null']
            script = f"import subprocess,sys; subprocess.Popen([sys.executable,{str(writer)!r}],start_new_session=True).wait()"
            child = None
            try:
                subprocess.run(tm+['new-session','-d','-s','test',sys.executable,'-c',script],env=dict(os.environ,RFIG_CLEANUP_HOME=temp),check=True)
                deadline=time.monotonic()+5
                while not (home/'pid').exists() and time.monotonic()<deadline:time.sleep(.02)
                child=int((home/'pid').read_text())
                self.assertEqual(cleanup_tmux(tm,[home]),set())
                self.assertTrue(child not in _processes() or _processes()[child][2].startswith('Z'),'detached cache writer survived tmux cleanup')
            finally:
                subprocess.run(tm+['kill-server'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
                cleanup_process_tree([child] if child else [],[writer])

    def test_process_table_contains_current_process(self):
        import os
        self.assertIn(os.getpid(), _processes())

    def test_cleanup_kills_nested_process_after_parent_exits(self):
        root = Path(__file__).resolve().parent / ".cleanup-fixture"
        root.mkdir(exist_ok=True)
        child = subprocess.Popen(
            [sys.executable, "-c", "import subprocess,time; subprocess.Popen(['sleep','30']); time.sleep(30)"],
            cwd=root,
        )
        time.sleep(0.1)
        self.assertEqual(cleanup_process_tree([child.pid], [root]), set())
        self.assertIsNotNone(child.poll())
        root.rmdir()


if __name__ == "__main__":
    unittest.main()
