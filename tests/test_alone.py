#!/usr/bin/env python3
"""Self-test for `checks.alone` (K6): a gate's `[alone]` binaries run with every check slot on the machine held.

    python3 tests/test_alone.py         (no cargo, no kit.toml reads past import; a few seconds)

What it holds the lock to: the solo phase waits for another worktree's gate to give up its slot, nobody
takes a slot while it runs, two gates reaching it at once both get through (no deadlock: neither waits while
holding a slot), the gate has its own slot back after, and one slot (KIT_CHECK_SLOTS=1) still works.
"""
from __future__ import annotations

import os
import subprocess as sp
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

ok = fail = 0


def check(cond, what: str) -> None:
    global ok, fail
    print(('  ok   ' if cond else '  FAIL ') + what)
    ok, fail = ok + bool(cond), fail + (not cond)


# A gate in another worktree: take a slot, then (optionally) hold it a while or run an alone phase.
GATE = '''
import sys, time
sys.path.insert(0, {root!r})
from ps2kit import checks
mode, hold = sys.argv[1], float(sys.argv[2])
checks._take_slot()
print('took', time.time(), flush=True)
if mode == 'alone':
    with checks.alone():
        print('alone+', time.time(), flush=True)
        time.sleep(hold)
        print('alone-', time.time(), flush=True)
else:
    time.sleep(hold)
print('gave', time.time(), flush=True)
checks._release_slot()
'''


def gate(env, mode: str, hold: float) -> sp.Popen:
    return sp.Popen([sys.executable, '-c', GATE.format(root=str(ROOT)), mode, str(hold)], env=env,
                    stdout=sp.PIPE, stderr=sp.DEVNULL, text=True)


def stamps(p: sp.Popen) -> dict[str, float]:
    out, _ = p.communicate(timeout=60)
    return {k: float(v) for k, v in (l.split() for l in out.splitlines())}


def wait_took(p: sp.Popen) -> float:
    k, v = p.stdout.readline().split()
    assert k == 'took'
    return float(v)


def test_alone() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        env = {**os.environ, 'XDG_RUNTIME_DIR': tmp, 'KIT_CHECK_SLOTS': '3'}
        env.pop('KIT_PCSX2', None)

        other = gate(env, 'hold', 1.5)
        wait_took(other)
        me = gate(env, 'alone', 1.0)
        took = wait_took(me)
        time.sleep(0.3)
        free = sp.run([sys.executable, '-c', f'import sys; sys.path.insert(0, {str(ROOT)!r}); from ps2kit import checks;'
                       ' l = checks._slot(block=False);'
                       ' print("free" if l else "none")'], env=env, capture_output=True, text=True).stdout.strip()
        rest = stamps(other)
        m = {'took': took, **stamps(me)}
        check(m['alone+'] >= rest['gave'], "the solo phase waits for the other worktree's gate to give its slot up")
        check(free == 'free', 'a slot stays free for a new gate while the solo phase waits (it gave its own up)')
        check('gave' in m, 'and the gate ends, its slot back')

        me = gate(env, 'alone', 1.0)
        wait_took(me)
        time.sleep(0.5)
        probe = sp.run([sys.executable, '-c', f'import sys; sys.path.insert(0, {str(ROOT)!r}); from ps2kit import checks;'
                        ' l = checks._slot(block=False); print("free" if l else "none")'],
                       env=env, capture_output=True, text=True).stdout.strip()
        stamps(me)
        check(probe == 'none', 'no slot can be taken while the solo phase runs')

        a, b = gate(env, 'alone', 0.8), gate(env, 'alone', 0.8)
        try:
            sa, sb = stamps(a), stamps(b)
            first, second = sorted([sa, sb], key=lambda s: s['alone+'])
            check(second['alone+'] >= first['alone-'], 'two gates reaching it at once both get through, one after the other')
        except sp.TimeoutExpired:
            a.kill(), b.kill()
            check(False, 'two gates reaching it at once deadlocked')

        one = {**env, 'KIT_CHECK_SLOTS': '1'}
        try:
            s = stamps(gate(one, 'alone', 0.1))
            check('gave' in s, 'one slot: the solo phase runs and the gate ends')
        except sp.TimeoutExpired:
            check(False, 'one slot: the solo phase never ran')


if __name__ == '__main__':
    test_alone()
    print(f'\n{ok} passed, {fail} failed')
    sys.exit(1 if fail else 0)
