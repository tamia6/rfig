"""Keep the Docker entry point stable; use the same real Linux sandbox assertions."""
from pathlib import Path
import runpy
runpy.run_path(str(Path(__file__).resolve().parents[1] / 'linux_sandbox.py'), run_name='__main__')
