"""Best-effort cleanup for real-shell tests on macOS and Linux."""

import os
import signal
import subprocess
import time


def _processes():
    rows = {}
    output = subprocess.check_output(["ps", "-axo", "pid=,ppid=,stat=,command="], text=True)
    for line in output.splitlines():
        fields = line.strip().split(None, 2)
        if len(fields) == 4:
            rows[int(fields[0])] = (int(fields[1]), fields[3], fields[2])
    return rows


def _descendants(processes, roots):
    found = set(roots)
    changed = True
    while changed:
        changed = False
        for pid, (ppid, _, _) in processes.items():
            if ppid in found and pid not in found:
                found.add(pid)
                changed = True
    return found


def cleanup_process_tree(pids=(), roots=(), timeout=3.0):
    """Terminate test-owned process trees, returning anything still alive."""
    processes = _processes()
    selected = set(pids)
    needles = tuple(str(root) for root in roots)
    selected.update(
        pid for pid, (_, command) in processes.items()
        if needles and any(needle in command for needle in needles)
    )
    selected = _descendants(processes, selected)
    selected.discard(os.getpid())
    for pid in selected:
        try:
            os.kill(pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        alive = {pid for pid, (_, _, stat) in _processes().items() if not stat.startswith("Z")} & selected
        if not alive:
            return set()
        time.sleep(0.05)
    for pid in {pid for pid, (_, _, stat) in _processes().items() if not stat.startswith("Z")} & selected:
        try:
            os.kill(pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    time.sleep(0.1)
    return {pid for pid, (_, _, stat) in _processes().items() if not stat.startswith("Z")} & selected


def cleanup_tmux(tmux, roots=()):
    """Kill tmux and every pane descendant before temporary HOME disappears."""
    pane_pids = []
    try:
        output = subprocess.check_output(
            tmux + ["list-panes", "-a", "-F", "#{pane_pid}"],
            text=True,
            stderr=subprocess.DEVNULL,
        )
        pane_pids = [int(line) for line in output.splitlines() if line.strip().isdigit()]
    except (OSError, subprocess.CalledProcessError):
        pass
    subprocess.run(tmux + ["kill-server"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    return cleanup_process_tree(pane_pids, roots)
