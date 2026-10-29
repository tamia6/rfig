"""Run the shared history-menu contract through native ZLE."""
import os
from pathlib import Path
import runpy
os.environ['RFIG_TEST_SHELL'] = 'zsh'
os.environ['RFIG_TEST_SHELL_EXE'] = '/bin/zsh'
runpy.run_path(str(Path(__file__).with_name('history_menu.py')), run_name='__main__')
