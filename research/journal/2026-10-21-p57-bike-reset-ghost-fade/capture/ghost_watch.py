#!/usr/bin/env python3
r"""P57: watch the bike's hand-reset ghost fade on the live game.

    ./kit pcsx2 with -- python3 -B research/journal/2026-10-21-p57-bike-reset-ghost-fade/capture/ghost_watch.py [--state 8]

Loads the state, waits for the ride to settle, finds the human's ridden actor (type 2, control source 1)
and prints the ghost fields once a tick, from a few ticks before an L1+R1 press on the kit pad until the
fade has run out:
  +0x7dc  the ghost clock, seconds left (counts down by the update's dt)
  +0x7e0  its length
  +0xb7c  1 = solid, 2 = ghost
  bike model (+0x7a0 -> +0x30) +0x16c  the draw alpha, and +0x136 bit 0x10 / +0x148 the see-through flag
  the app's tick counter, so the clock's step per tick is dt
Research, not product.
"""

import argparse
import os
import subprocess
import sys
import time

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..', '..', '..'))
sys.path.insert(0, ROOT)

from ps2kit.pine import Pine  # noqa: E402

GAME_MANAGER = 0x004E1E98
FRAME_COUNTER = 0x00479D04
T0 = [0.0]


def chain(p, addr, *offs):
    for off in offs:
        addr = p.read32(addr)
        if not addr or addr < 0x100000 or addr >= 0x2000000:
            return 0
        addr += off
    return addr


def human_bike(p):
    node = p.read32(chain(p, GAME_MANAGER, 0x34, 0x30 + 4))
    for _ in range(64):
        if not node:
            break
        item = p.read32(node)
        if item and p.read32(item) == 2:
            player = p.read32(item + 0x20)
            if player and p.read32(player + 0x34) == 1:
                return item
        nxt = p.read32(node + 8)
        if nxt == node:
            break
        node = nxt
    return 0


def pad(*cmds):
    subprocess.run([os.path.join(ROOT, 'kit'), 'pad', 'send', *cmds], cwd=ROOT, check=True)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument('--state', type=int, default=8)
    ap.add_argument('--ticks', type=int, default=260)
    a = ap.parse_args()
    p = Pine()
    p.load_state(a.state)
    time.sleep(2.0)
    pad('release', 'sleep 1500')
    bike = human_bike(p)
    if not bike:
        print('no human bike')
        return 1
    model = p.read32(p.read32(bike + 0x7a0) + 0x30)
    n_parts = p.read32(bike + 0x950)
    parts = [p.read32(p.read32(bike + 0x954) + 4 * i) for i in range(n_parts)]
    print(f'bike 0x{bike:08x} model 0x{model:08x} parts {[hex(x) for x in parts]}')
    print(f'length +0x7e0 = {p.read_f32(bike + 0x7e0)}')

    def row(tag):
        f = p.read32(FRAME_COUNTER)
        clock = p.read_f32(bike + 0x7dc)
        state = p.read32(bike + 0xb7c)
        alpha = p.read_f32(model + 0x16c)
        flag = p.read8(model + 0x136) & 0x10
        k = p.read_f32(model + 0x148)
        pa = [round(p.read_f32(x + 0x16c), 4) for x in parts]
        print(f'{tag} tick {f} clock {clock:.6f} state {state} alpha {alpha:.6f} flag {flag:#x} '
              f'k {k} parts {pa} kind {p.read32(bike + 0x14d4)} '
              f'ride {p.read32(bike + 0x748)} {p.read32(bike + 0x74c)} {p.read_f32(bike + 0x794):.3f} latch {p.read32(bike + 0x14d0)} pos {[round(p.read_f32(bike + 0x5e0 + 4 * i), 2) for i in range(3)]} '
              f'bits {p.read32(bike + 0x7dc):08x} {p.read32(model + 0x16c):08x} t {time.time() - T0[0]:.3f}')
        return f, clock

    T0[0] = time.time()
    last = None
    for _ in range(5):
        last = row('pre ')
    print(f'chord at {time.time() - T0[0]:.3f}')
    pad('down l1', 'down r1', os.environ.get('HOLD', 'sleep 120'), 'up l1', 'up r1')
    print(f'chord sent {time.time() - T0[0]:.3f}')
    seen = 0
    prev = None
    t0 = time.time()
    while seen < a.ticks and time.time() - t0 < 12:
        f = p.read32(FRAME_COUNTER)
        if f == prev:
            continue
        prev = f
        _, clock = row('post')
        seen += 1
        if clock <= 0 and seen > 120:
            break
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
