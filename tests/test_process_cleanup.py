import subprocess
import sys
import time
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from process_cleanup import cleanup_process_tree


class ProcessCleanupTests(unittest.TestCase):
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
