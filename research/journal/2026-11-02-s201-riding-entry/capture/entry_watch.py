#!/usr/bin/env python3
r"""S201: S74's bones capture (`bones_watch.py`, unchanged regions and pad scripts) plus the rider body's
eighteen part bodies, once a tick, to see what the riding entry after a put-back starts them with.

    KIT_PCSX2=3 ./kit pcsx2 with -- python3 -B research/journal/2026-11-02-s201-riding-entry/capture/entry_watch.py \
        --state 6 --script ride --out context/s201/free06-ride.txt

Added to S74's regions:

  body.2a4  the ragdoll's riding/repose/placed/active/stiff words (+0x2a4..+0x2b7)
  body.4d0  the holds' targets (+0x4d0..), the carried velocity +0x530, the bike box +0x540/+0x550 and
            the drawn bike transform +0x580..+0x5bf, up to the hold pins +0x5d0..+0x5df
  pin<k>    hold k's pin (*(body+0x5d0+4k)) +0x40..+0x5f: its target and that target's velocity
  start<i>  part i's body +0x30..+0x4f: the step's start position and velocity
  part<i>   part i's body (`*(*(body+0x2f4) + 0xb0 i + 0xa4)`, S109's layout) +0x50..+0x1bf: last
            position +0x50, position +0x60, velocity +0x70, last quaternion +0x150, quaternion +0x170,
            rates +0x1b0

Loads the state, finds the human's bike (type 2, control source 1) and its rider body actor (the type-4
record less 0x400 whose request block +0x488 is the bike's +0xb80), and logs every tick, as hex, while
S66's pad scripts play on the kit pad in a thread:

  bike     +0x748 reason/latch, +0x794 clock, +0x7dc ghost clock, +0x14d0 chord latch/kind, +0x3b0 the
           put-back record, the bike node's local and world rows (+0x40..+0xbf)
  pad      the control states
  body     +0x3c..+0x9b the motion object's clock, +0x47c..+0x493 mode/prev/request/touching/roll,
           +0x680 the machine's flags, +0x8dc its cross/state words, +0x9f0, +0xa14 bands, +0xa3c,
           +0x6a8..+0x6c3 the wreck fields, the two axis lag records behind +0x630/+0x634
  request  the 0x1b0 bytes behind +0x488
  joints   the 22 joint records (0x60 each: +0x40 translation, +0x50 quaternion, +0x34 the node)
  snapshot the 22 quaternions behind +0x248 and the 22 translations behind +0x24c
  nodes    each joint's node +0x00..+0xbf: flags word, local rows +0x40..+0x7f, world rows +0x80..+0xbf

Each line is `<name> <hex>`; a tick starts with `f <frame>`. Research, not product.
"""

import argparse
import os
import subprocess
import sys
import threading
import time

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..', '..', '..'))
sys.path.insert(0, ROOT)
sys.path.insert(0, os.path.dirname(__file__))
sys.path.insert(0, os.path.join(ROOT, 'research', 'journal', '2026-10-21-s74-putback-bones', 'capture'))

from ps2kit.pine import Pine  # noqa: E402
from bones_common import CONTROLS, FRAME_COUNTER, FRAME_DT, human_bike, rider_body  # noqa: E402

SCRIPTS = {
    # S66's three, unchanged, so the put-backs fall where S66's did.
    'ride': ['release', 'sleep 1500', 'down cross', 'sleep 5000', 'down l1', 'sleep 120', 'down r1',
             'sleep 200', 'up l1', 'up r1', 'sleep 4000', 'down l1', 'sleep 120', 'down r1', 'sleep 200',
             'up l1', 'up r1', 'sleep 3000', 'release'],
    'down': ['release', 'sleep 1500', 'down cross', 'stick l 0 1', 'sleep 3500', 'stick l 1 1', 'sleep 2500',
             'stick l 0 0', 'up cross', 'sleep 1200', 'down l1', 'sleep 120', 'down r1', 'sleep 200',
             'up l1', 'up r1', 'sleep 4000', 'release'],
    'downchord': ['release', 'sleep 1500', 'down cross', 'stick l 0 1', 'sleep 4000', 'up cross', 'stick l 0 0',
                  'down l1', 'sleep 120', 'down r1', 'sleep 200', 'up l1', 'up r1', 'sleep 4000', 'release'],
}


def pad(cmds):
    subprocess.run([os.path.join(ROOT, 'kit'), 'pad', 'send', *cmds], cwd=ROOT, check=True)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument('--state', type=int, required=True)
    ap.add_argument('--script', choices=sorted(SCRIPTS), default='ride')
    ap.add_argument('--seconds', type=float, default=18.0)
    ap.add_argument('--out', required=True)
    # With no virtual pad (no /dev/uinput), a tape plays the script instead (`make_tape.py`): attach
    # waits up to this many seconds for the tape's state to bring the rider in, and loads nothing.
    ap.add_argument('--attach', type=float, default=0.0)
    ap.add_argument('--after', type=int, default=0, help='with --attach: the first frame to log')
    a = ap.parse_args()
    os.makedirs(os.path.dirname(a.out), exist_ok=True)
    p = Pine()
    if a.attach:
        # The tape's load is the frame counter jumping (the Reset before it zeroes the counter).
        t0 = time.time()
        bike = body = 0
        last, loaded, seen = None, False, []
        while time.time() - t0 < a.attach:
            f = p.read32(FRAME_COUNTER)
            if last is not None and abs(f - last) > 100:
                loaded = True
            if f != last:
                seen.append(f)
            last = f
            if loaded and f >= a.after:
                bike = human_bike(p)
                body = rider_body(p, bike) if bike else 0
                if body:
                    break
            time.sleep(0.1)
    else:
        p.load_state(a.state)
        t0 = time.time()
        while time.time() - t0 < 2.5:
            time.sleep(0.05)
        bike = human_bike(p)
        body = rider_body(p, bike) if bike else 0
    if not body:
        print('no human bike or rider body', 'frames seen', locals().get('seen', [])[::10])
        return 1
    bnode = p.read32(bike + 0x18)
    req = p.read32(body + 0x488)
    n = p.read32(body + 0x230)
    arr = p.read32(body + 0x234)
    ur = p.read32(body + 0x248)
    ut = p.read32(body + 0x24c)
    lag_mix = p.read32(body + 0x630)
    lag_phase = p.read32(body + 0x634)
    nodes = [p.read32(arr + 0x60 * j + 0x34) for j in range(n)]
    parts_at = p.read32(body + 0x2f4)
    parts = [p.read32(parts_at + 0xb0 * i + 0xa4) for i in range(p.read32(body + 0x2e0))]
    out = open(a.out, 'w')
    out.write(f'# bike {bike:08x} bike-node {bnode:08x} body {body:08x} req {req:08x} joints {n} arr {arr:08x} '
              f'ur {ur:08x} ut {ut:08x} lags {lag_mix:08x} {lag_phase:08x} nodes '
              + ' '.join(f'{x:08x}' for x in nodes) + f' parts ' + ' '.join(f'{x:08x}' for x in parts) + f' script {a.script} state {a.state}\n')
    out.write('# clip-table ' + p.read_block(body + 0x900, 240).hex() + '\n')
    regions = [
        ('dt', FRAME_DT, 4),
        ('bike.748', bike + 0x748, 0x10), ('bike.794', bike + 0x794, 4), ('bike.7dc', bike + 0x7dc, 4),
        ('bike.14d0', bike + 0x14d0, 8), ('bike.3b0', bike + 0x3b0, 0x30), ('bike.node', bnode + 0x40, 0x80),
        ('pad', CONTROLS, 0x18 * 6),
        ('body.3c', body + 0x3c, 0x60), ('body.47c', body + 0x47c, 0x18), ('body.680', body + 0x680, 40),
        ('body.8dc', body + 0x8dc, 36), ('body.9f0', body + 0x9f0, 8), ('body.a14', body + 0xa14, 12),
        ('body.a3c', body + 0xa3c, 4), ('body.6a8', body + 0x6a8, 0x1c), ('body.2a4', body + 0x2a4, 0x14),
        ('body.4d0', body + 0x4d0, 0x110),
        ('lag.mix', lag_mix, 12), ('lag.phase', lag_phase, 12),
        ('req', req, 0x1b0),
        ('joints', arr, 0x60 * n),
        ('ur', ur, 0x10 * n) if ur else None, ('ut', ut, 0x10 * n) if ut else None,
    ]
    regions = [r for r in regions if r] + [(f'node{j}', nodes[j], 0xc0) for j in range(n)]
    regions += [(f'part{i}', a + 0x50, 0x170) for i, a in enumerate(parts)]
    regions += [(f'start{i}', a + 0x30, 0x20) for i, a in enumerate(parts)]
    regions += [(f'pin{k}', p.read32(body + 0x5d0 + 4 * k) + 0x40, 0x20) for k in range(4)]
    spans = [(addr, ln) for _, addr, ln in regions]
    th = threading.Thread(target=pad, args=(SCRIPTS[a.script],))
    if not a.attach:
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
        if prev is None:
            t0 = time.time()  # attached, the VM sits paused after the tape's load until resumed
        prev = f
        out.write(f'f {f}\n')
        o = 0
        for name, _, ln in regions:
            out.write(f'{name} {b[o:o + ln].hex()}\n')
            o += ln
    if not a.attach:
        th.join()
    out.write(f'# torn {torn} gaps {gaps}\n')
    out.close()
    print(f'wrote {a.out}, torn {torn}, gaps {gaps}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
