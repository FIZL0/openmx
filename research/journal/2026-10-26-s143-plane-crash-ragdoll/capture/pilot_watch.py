#!/usr/bin/env python3
r"""S143: the biplane's pilot through a crash, once a tick.

    ./kit pcsx2 with -- python3 -B research/journal/2026-10-26-s143-plane-crash-ragdoll/capture/pilot_watch.py \
        --state 7 --script nose --out context/s143/nose.txt

Loads the state (the biplane recording's, the plane on the runway), finds the human's plane (actor type
1, control source 1) and its pilot's body (the type-4 record less 0x400 whose request +0x488 is the
plane's +0x690), and logs every tick, as hex, while a pad script plays on the kit pad in a thread:

  plane    +0x3b0..+0x3bf wreck reason/latch/clock, +0x590 grounded, +0x598 airspeed, the plane node's
           world rows, +0x330..+0x36f the pad, throttle and surfaces, the model's root node's world rows
           (*(+0x18c) + 0x80), the rigid body's +0x50..+0x7f (pos, pos, vel) and +0x150..+0x1bf (rotation, spin)
  body     the pilot body as S74's bones_watch.py reads a rider's (+0x3c, +0x47c, +0x680, +0x6a8, ...)
  req      the 0x1b0 bytes behind +0x488
  joints / nodes  the 22 joint records and each joint node's +0x00..+0xbf

Each line is `<name> <hex>`; a tick starts with `f <frame>`. Research, not product.
"""

import argparse
import os
import subprocess
import sys
import threading
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, '..', '..', '..', '..'))
S74 = os.path.join(ROOT, 'research', 'journal', '2026-10-21-s74-putback-bones', 'capture')
sys.path.insert(0, ROOT)
sys.path.insert(0, S74)

from ps2kit.pine import Pine  # noqa: E402
from bones_common import CONTROLS, FRAME_COUNTER, FRAME_DT, actors  # noqa: E402

TAKEOFF = ['release', 'sleep 1000', 'down cross', 'sleep 7000', 'down down', 'sleep 2500', 'up down',
           'sleep 1500']
SCRIPTS = {
    # S57's first dive: straight down into the ground, wait the wreck out.
    'nose': TAKEOFF + ['down up', 'sleep 6000', 'up up', 'sleep 7000', 'up cross', 'release'],
    # Up, then a hard bank to the left held into the ground: a wing tip first.
    # The stick stays hard over through the wreck, so the pilot's lean shows whether it still follows.
    'wing': TAKEOFF + ['stick l -1 0', 'sleep 700', 'down up', 'sleep 3500', 'stick l -1 -1',
                       'sleep 7000', 'up up', 'stick l 0 0', 'up cross', 'release'],
    # Up, a short push to level low, then a shallow push in at full throttle: the belly (or the wheels)
    # at speed.
    'belly': TAKEOFF + ['down up', 'sleep 900', 'up up', 'sleep 2500', 'down up', 'sleep 400', 'up up',
                        'sleep 7000', 'up cross', 'release'],
    # The nose crash with the stick moved through the wreck: hard left, hard right, full back, full
    # forward, a second each, to see whether the pilot's lean still follows the stick once wrecked.
    'waggle': TAKEOFF + ['down up', 'sleep 6000', 'up up', 'sleep 1500', 'stick l -1 0', 'sleep 1000',
                         'stick l 1 0', 'sleep 1000', 'stick l 0 1', 'sleep 1000', 'stick l 0 -1', 'sleep 1000',
                         'stick l 0 0', 'sleep 2500', 'up cross', 'release'],
}


def pad(cmds):
    subprocess.run([os.path.join(ROOT, 'kit'), 'pad', 'send', *cmds], cwd=ROOT, check=True)


def human_plane(p):
    for item, kind in actors(p):
        if kind == 1:
            player = p.read32(item + 0x20)
            if player and p.read32(player + 0x34) == 1:
                return item
    return 0


def pilot_body(p, plane):
    seen = []
    for item, kind in actors(p):
        if kind == 4:
            body = item - 0x400
            req = p.read32(body + 0x488)
            seen.append((item, req))
            if req == plane + 0x690:
                return body
    print('type-4 records and their requests:', [(f'{i:08x}', f'{r:08x}') for i, r in seen])
    return 0


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument('--state', type=int, required=True)
    ap.add_argument('--script', choices=sorted(SCRIPTS), default='nose')
    ap.add_argument('--seconds', type=float, default=26.0)
    ap.add_argument('--out', required=True)
    a = ap.parse_args()
    os.makedirs(os.path.dirname(a.out), exist_ok=True)
    p = Pine()
    p.load_state(a.state)
    t0 = time.time()
    while time.time() - t0 < 2.5:
        time.sleep(0.05)
    plane = human_plane(p)
    print(f'plane {plane:08x}', [(f'{i:08x}', k) for i, k in actors(p)])
    body = pilot_body(p, plane) if plane else 0
    if not body:
        print('no human plane or pilot body')
        return 1
    pnode = p.read32(plane + 0x18)
    model = p.read32(plane + 0x18c)
    rigid = p.read32(plane + 0x160)
    req = p.read32(body + 0x488)
    n = p.read32(body + 0x230)
    arr = p.read32(body + 0x234)
    nodes = [p.read32(arr + 0x60 * j + 0x34) for j in range(n)]
    out = open(a.out, 'w')
    out.write(f'# plane {plane:08x} plane-node {pnode:08x} rigid {rigid:08x} body {body:08x} req {req:08x} '
              f'joints {n} arr {arr:08x} nodes ' + ' '.join(f'{x:08x}' for x in nodes)
              + f' script {a.script} state {a.state}\n')
    regions = [
        ('dt', FRAME_DT, 4),
        ('plane.3b0', plane + 0x3b0, 0x10), ('plane.590', plane + 0x590, 0x10),
        ('plane.node', pnode + 0x40, 0x80), ('plane.330', plane + 0x330, 0x40),
        ('model.node', model + 0x80, 0x40),
        ('rigid.50', rigid + 0x50, 0x30), ('rigid.150', rigid + 0x150, 0x70),
        ('pad', CONTROLS, 0x18 * 6),
        ('body.3c', body + 0x3c, 0x60), ('body.47c', body + 0x47c, 0x18), ('body.680', body + 0x680, 40),
        ('body.8dc', body + 0x8dc, 36), ('body.9f0', body + 0x9f0, 8), ('body.a14', body + 0xa14, 12),
        ('body.a3c', body + 0xa3c, 4), ('body.6a8', body + 0x6a8, 0x1c),
        ('req', req, 0x1b0),
        ('joints', arr, 0x60 * n),
    ] + [(f'node{j}', nodes[j], 0xc0) for j in range(n)]
    spans = [(addr, ln) for _, addr, ln in regions]
    th = threading.Thread(target=pad, args=(SCRIPTS[a.script],))
    th.start()
    prev = None
    t0 = time.time()
    torn = 0
    gaps = 0
    while time.time() - t0 < a.seconds:
        f = p.read32(FRAME_COUNTER)
        if f == prev:
            time.sleep(0.002)
            continue
        time.sleep(0.004)
        b = p.settle(spans, f)
        if b is None:
            torn += 1
            continue
        if prev is not None and f != prev + 1:
            gaps += 1
        prev = f
        out.write(f'f {f}\n')
        o = 0
        for name, _, ln in regions:
            out.write(f'{name} {b[o:o + ln].hex()}\n')
            o += ln
    th.join()
    out.write(f'# torn {torn} gaps {gaps}\n')
    out.close()
    print(f'wrote {a.out}, torn {torn}, gaps {gaps}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
