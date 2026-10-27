"""agent-runner from `uv tool install`. uv keeps it in a venv of its own, which the system Python that runs the
kit (it has the distribution's evdev) cannot import from: `add()` appends that venv's site-packages to sys.path,
and `PYTHON` is the venv's interpreter, for the runner's calls to itself (`self_calls()`).
"""
from __future__ import annotations

import os
import sys
from pathlib import Path


def _tools() -> Path:
    if d := os.environ.get('UV_TOOL_DIR'):
        return Path(d)
    return Path(os.environ.get('XDG_DATA_HOME') or Path.home() / '.local' / 'share') / 'uv' / 'tools'


VENV = _tools() / 'agent-runner'
PYTHON = VENV / 'bin' / 'python'


def add() -> None:
    """Put the tool's site-packages last on sys.path (a copy installed for this Python still wins)."""
    for sp in sorted(VENV.glob('lib/python3.*/site-packages')):
        if str(sp) not in sys.path:
            sys.path.append(str(sp))


def self_calls() -> None:
    """The Stop hook, slot shim and stop-waiter the runner writes are `<python> -m agent_runner …`, `<python>`
    being whatever runs the kit; with the tool installed, make that the tool's Python, which has the package."""
    if not PYTHON.exists():
        return
    from agent_runner import config as C
    cmd = lambda: [str(PYTHON), '-m', 'agent_runner']
    C.runner_cmd = cmd
    if R := sys.modules.get('agent_runner.runner'):
        R.runner_cmd = cmd  # it imported the name; one imported later picks up the patched one
