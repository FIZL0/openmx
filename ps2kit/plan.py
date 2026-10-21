"""`./kit plan`: the task list as a tool. agent-runner's plan (https://github.com/FIZL0/agent-runner) does
the work, with the kit's settings (see ps2kit/runner.py); `./kit plan --help` lists the subcommands.
"""
from __future__ import annotations

import os
import sys

from ps2kit.config import ROOT


def _plan():
    """agent-runner's plan module, pointed at this checkout and the kit's settings."""
    from agent_runner import plan as P
    from ps2kit.runner import use
    use()
    os.environ.setdefault('AGENT_RUNNER_REPO', str(ROOT))  # a test points it at a scratch repo
    P.PROG = './kit plan'
    return P


def tick(tid: str) -> int:
    return _plan().tick(tid)


def block(tid: str, why: str, after=None) -> None:
    _plan().block(tid, why, after=after)


def main(argv: list[str] | None = None) -> int:
    argv = list(sys.argv[1:] if argv is None else argv)
    P = _plan()
    if argv[:1] in (['-h'], ['--help'], ['help']):
        print(P.HELP.replace('./run plan', './kit plan').replace('./run run', './kit run').rstrip())
        return 0
    return P.main(argv)
