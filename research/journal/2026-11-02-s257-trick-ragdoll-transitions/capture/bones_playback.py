#!/usr/bin/env python3
r"""S257: S74's bones rows (bones_watch.py's regions) through a .p2m2 played back, once a tick.

    python3 -B bones_playback.py FIRST N OUT

No state load and no pad: the playback drives the game (play.sh starts it). Waits for frame_counter in
FIRST..FIRST+26 with the human's bike up, finds the bike and its rider body actor as bones_watch.py does,
then logs N ticks in bones_watch.py's format, plus `trick` (the player record's +0xa0c running and
+0xa78 id), `body.2a4` (the body's +0x2a4 riding, +0x2a8 repose, +0x2ac placed, +0x2b0 active), and per
part i of the ragdoll (`*(body+0x2f4) + 0xb0 i`, S109's layout) `prec<i>` the part record (+0x60 the drawn
matrix the pose measures velocity against), `start<i>` its body's +0x30..+0x4f (the step's start
position and velocity, S201) and `pbody<i>` its body's +0x50..+0x7f (position, velocity).
Research, not product.
"""

import os
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, '..', '..', '..', '..'))
S74 = os.path.join(ROOT, 'research', 'journal', '2026-10-21-s74-putback-bones', 'capture')
sys.path.insert(0, ROOT)
sys.path.insert(0, S74)

from ps2kit.pine import Pine  # noqa: E402
from bones_common import CONTROLS, FRAME_COUNTER, FRAME_DT, GAME_MANAGER, chain, human_bike, rider_body  # noqa: E402


def main() -> int:
    first, n, out_path = int(sys.argv[1]), int(sys.argv[2]), sys.argv[3]
    os.makedirs(os.path.dirname(os.path.abspath(out_path)), exist_ok=True)
    p = Pine()
    deadline = time.time() + 240
    bike = body = 0
    while time.time() < deadline:
        f = p.read32(FRAME_COUNTER)
        if first <= f <= first + 26:
            bike = human_bike(p)
            body = rider_body(p, bike) if bike else 0
            if body:
                break
        time.sleep(0.01)
    if not body:
        print('CAPTURE FAILED: never saw the human bike and its body in the window', flush=True)
        return 1
    player = p.read32(bike + 0x20)
    bnode = p.read32(bike + 0x18)
    req = p.read32(body + 0x488)
    nj = p.read32(body + 0x230)
    arr = p.read32(body + 0x234)
    lag_mix = p.read32(body + 0x630)
    lag_phase = p.read32(body + 0x634)
    nodes = [p.read32(arr + 0x60 * j + 0x34) for j in range(nj)]
    parts_at = p.read32(body + 0x2f4)
    parts = [p.read32(parts_at + 0xb0 * i + 0xa4) for i in range(p.read32(body + 0x2e0))]
    out = open(out_path, 'w')
    out.write(f'# bike {bike:08x} player {player:08x} bike-node {bnode:08x} body {body:08x} req {req:08x} joints {nj} '
              f'arr {arr:08x} nodes ' + ' '.join(f'{x:08x}' for x in nodes) + ' playback\n')
    out.write('# clip-table ' + p.read_block(body + 0x900, 240).hex() + '\n')
    regions = [
        ('dt', FRAME_DT, 4),
        ('bike.748', bike + 0x748, 0x10), ('bike.794', bike + 0x794, 4), ('bike.7dc', bike + 0x7dc, 4),
        ('bike.14d0', bike + 0x14d0, 8), ('bike.node', bnode + 0x40, 0x80),
        ('trick', player + 0xa0c, 4), ('trick.id', player + 0xa78, 4),
        ('pad', CONTROLS, 0x18 * 6),
        ('body.2a4', body + 0x2a4, 0x18),
        ('body.3c', body + 0x3c, 0x60), ('body.47c', body + 0x47c, 0x18), ('body.680', body + 0x680, 40),
        ('body.8dc', body + 0x8dc, 36), ('body.9f0', body + 0x9f0, 8), ('body.a14', body + 0xa14, 12),
        ('body.a3c', body + 0xa3c, 4), ('body.6a8', body + 0x6a8, 0x1c),
        ('lag.mix', lag_mix, 12), ('lag.phase', lag_phase, 12),
        ('req', req, 0x1b0),
        ('joints', arr, 0x60 * nj),
    ] + [(f'node{j}', nodes[j], 0xc0) for j in range(nj)]
    regions += [(f'prec{i}', parts_at + 0xb0 * i, 0xb0) for i in range(len(parts))]
    regions += [(f'start{i}', a + 0x30, 0x20) for i, a in enumerate(parts)]
    regions += [(f'pbody{i}', a + 0x50, 0x30) for i, a in enumerate(parts)]
    spans = [(addr, ln) for _, addr, ln in regions]
    prev = None
    torn = gaps = rows = 0
    deadline = time.time() + n / 25.0 + 60
    while rows < n and time.time() < deadline:
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
        rows += 1
        out.write(f'f {f}\n')
        o = 0
        for name, _, ln in regions:
            out.write(f'{name} {b[o:o + ln].hex()}\n')
            o += ln
    out.write(f'# torn {torn} gaps {gaps} rows {rows}\n')
    out.close()
    print(f'wrote {out_path}, rows {rows}, torn {torn}, gaps {gaps}', flush=True)
    return 0 if rows == n else 1


if __name__ == '__main__':
    raise SystemExit(main())
