#!/usr/bin/env python3
r"""S66: watch where the original puts the bike back on L1+R1.

    ./kit pcsx2 with -- python3 -B research/journal/2026-10-21-s66-hand-reset-position/capture/putback_watch.py \
        --state 6 --script ride --out context/s66/run1.txt

Loads the state, finds the human's ridden actor (type 2, control source 1) and logs, once a tick, every
field the put-back reads or writes, while a pad script plays on the kit pad in a thread:
  +0x3b0/+0x3c0/+0x3d0  the put-back record: point, forward (flat), up — what the model setup applies
  +0x3e0/+0x3f0/+0x400  the spawn record the constructor wrote (and message 3 copies back)
  +0x1350..+0x137c      the checkpoint copy hand reset takes in race modes
  +0x5e0                the point the model setup last applied
  +0x748 +0x74c +0x794  reason, latch, clock;  +0x14d0 chord latch, +0x14d4 kind;  +0x7dc ghost clock
  +0xc84                (read by the record's updater: 13 skips the probe)
  node (+0x18) +0x40..+0xbc  local rows (translation +0x70) and world rows (+0xb0)
  body (+0x7a4) +0x60 centre, +0x260 velocity, +0x280 angular velocity
  pad0 control state: held for Cross, L1, R1
  collision record (+0x7d0) +0xb0..+0xef: the sweep's offset and segment; +0xe0 is the probe's x/z riding
  *(+0x7a8)+0x124 +0x40: the point the probe uses while down;  race +0x60 and +0x120: what the race fills
Every float is printed as its bit pattern too, so a fixture can be bit-exact. Research, not product.
"""

import argparse
import os
import struct
import subprocess
import sys
import threading
import time

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..', '..', '..'))
sys.path.insert(0, ROOT)

from ps2kit.pine import Pine  # noqa: E402

GAME_MANAGER = 0x004E1E98
FRAME_COUNTER = 0x00479D04
FRAME_DT = 0x00479CD4
CONTROLS = 0x00678F08

SCRIPTS = {
    # Gas from the start for a few seconds, the chord while riding on, a few seconds more, the chord again.
    'ride': ['release', 'sleep 1500', 'down cross', 'sleep 5000', 'down l1', 'sleep 120', 'down r1',
             'sleep 200', 'up l1', 'up r1', 'sleep 4000', 'down l1', 'sleep 120', 'down r1', 'sleep 200',
             'up l1', 'up r1', 'sleep 3000', 'release'],
    # Gas and a hard turn so the rider goes down, then the chord while down.
    'down': ['release', 'sleep 1500', 'down cross', 'stick l 0 1', 'sleep 3500', 'stick l 1 1', 'sleep 2500',
             'stick l 0 0', 'up cross', 'sleep 1200', 'down l1', 'sleep 120', 'down r1', 'sleep 200',
             'up l1', 'up r1', 'sleep 4000', 'release'],
    # free06's own lip wrecks the rider about 4.5 s in (reason 11); the chord a second later, inside
    # MaxWreckTime, so it is the chord and not the automatic reset that puts the bike back.
    'downchord': ['release', 'sleep 1500', 'down cross', 'stick l 0 1', 'sleep 4000', 'up cross', 'stick l 0 0',
                  'down l1', 'sleep 120', 'down r1', 'sleep 200', 'up l1', 'up r1', 'sleep 4000', 'release'],
}


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


def pad(cmds):
    subprocess.run([os.path.join(ROOT, 'kit'), 'pad', 'send', *cmds], cwd=ROOT, check=True)


def vec(b, off, n=4):
    return [struct.unpack_from('<f', b, off + 4 * i)[0] for i in range(n)]


def bits(b, off, n=4):
    return ' '.join(f'{struct.unpack_from("<I", b, off + 4 * i)[0]:08x}' for i in range(n))


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument('--state', type=int, required=True)
    ap.add_argument('--script', choices=sorted(SCRIPTS), default='ride')
    ap.add_argument('--seconds', type=float, default=18.0)
    ap.add_argument('--out', required=True)
    a = ap.parse_args()
    os.makedirs(os.path.dirname(a.out), exist_ok=True)
    out = open(a.out, 'w')
    p = Pine()
    p.load_state(a.state)
    t0 = time.time()
    while time.time() - t0 < 2.5:
        time.sleep(0.05)
    bike = human_bike(p)
    if not bike:
        print('no human bike')
        return 1
    node = p.read32(bike + 0x18)
    body = p.read32(bike + 0x7a4)
    race = p.read32(bike + 0xf9c)
    coll = p.read32(bike + 0x7d0)
    rag = p.read32(bike + 0x7a8)
    part = p.read32(rag + 0x124) if rag else 0
    mode = p.read32(bike + 0xfa0)
    out.write(f'# bike {bike:08x} node {node:08x} body {body:08x} race {race:08x} mode-obj {mode:08x} '
              f'mode {p.read32(mode + 100) if mode else -1} rider-height +0x13a0 {p.read_f32(bike + 0x13a0)!r} '
              f'race-flags {p.read64(race + 8) if race else 0:#x} race+0x40 {p.read32(race + 0x40) if race else -1} '
              f'coll {coll:08x} rag {rag:08x} part {part:08x}\n')
    regions = [
        (FRAME_COUNTER, 4), (FRAME_DT, 4),
        (bike + 0x3b0, 0x60), (bike + 0x5e0, 0x10), (bike + 0x748, 0x10), (bike + 0x794, 4),
        (bike + 0x7dc, 4), (bike + 0xc84, 4), (bike + 0x1350, 0x30), (bike + 0x14d0, 8),
        (node + 0x40, 0x80), (body + 0x60, 0x10), (body + 0x260, 0x30),
        (CONTROLS, 0x18 * 6), (race + 8, 8),
        (coll + 0xb0, 0x40), (part + 0x40, 0x10), (race + 0x60, 0x10), (race + 0x120, 0x10),
    ]
    th = threading.Thread(target=pad, args=(SCRIPTS[a.script],))
    th.start()
    prev = None
    t0 = time.time()
    torn = 0
    while time.time() - t0 < a.seconds:
        f = p.read32(FRAME_COUNTER)
        if f == prev:
            time.sleep(0.002)
            continue
        time.sleep(0.004)
        b = p.settle(regions, f)
        if b is None:
            torn += 1
            continue
        prev = f
        o = 0
        frame, = struct.unpack_from('<I', b, 0)
        dt = vec(b, 4, 1)[0]
        o = 8
        rec = o; o += 0x60
        applied = o; o += 0x10
        st = o; o += 0x10
        clock = o; o += 4
        ghost = o; o += 4
        c84 = o; o += 4
        chk = o; o += 0x30
        latch = o; o += 8
        nd = o; o += 0x80
        centre = o; o += 0x10
        vel = o; o += 0x30
        ctl = o; o += 0x18 * 6
        rflags = o; o += 8
        sweep = o; o += 0x40
        partp = o; o += 0x10
        r60 = o; o += 0x10
        r120 = o
        reason, rlatch = struct.unpack_from('<II', b, st)
        chord, kind = struct.unpack_from('<II', b, latch)
        held = [struct.unpack_from('<I', b, ctl + 0x18 * i)[0] for i in (2, 4, 5)]
        fl, = struct.unpack_from('<Q', b, rflags)
        r = lambda v: '[' + ', '.join(f'{x:.4f}' for x in v) + ']'  # noqa: E731
        out.write(
            f'f {frame} dt {dt:.6f} x/l1/r1 {held} reason {reason} latch {rlatch} clk {vec(b, clock, 1)[0]:.3f} '
            f'chord {chord} kind {kind} ghost {vec(b, ghost, 1)[0]:.4f} c84 {struct.unpack_from("<I", b, c84)[0]} '
            f'rflags {fl:#x}\n'
            f'  rec.p {r(vec(b, rec))} {bits(b, rec)}\n'
            f'  rec.f {r(vec(b, rec + 0x10))} {bits(b, rec + 0x10)}\n'
            f'  rec.u {r(vec(b, rec + 0x20))} {bits(b, rec + 0x20)}\n'
            f'  spawn {r(vec(b, rec + 0x30))} fwd {r(vec(b, rec + 0x40))} up {r(vec(b, rec + 0x50))}\n'
            f'  chk.p {r(vec(b, chk))} f {r(vec(b, chk + 0x10))} u {r(vec(b, chk + 0x20))}\n'
            f'  applied {r(vec(b, applied))} {bits(b, applied)}\n'
            f'  node.l0 {r(vec(b, nd))} l1 {r(vec(b, nd + 0x10))} l2 {r(vec(b, nd + 0x20))} l3 {r(vec(b, nd + 0x30))}\n'
            f'  node.l3 {bits(b, nd + 0x30)}  l2 {bits(b, nd + 0x20)}  l1 {bits(b, nd + 0x10)}  l0 {bits(b, nd)}\n'
            f'  node.w3 {r(vec(b, nd + 0x70))}\n'
            f'  body.c {r(vec(b, centre))} vel {r(vec(b, vel))} ang {r(vec(b, vel + 0x20))}\n'
            f'  coll.b0 {r(vec(b, sweep))} c0 {r(vec(b, sweep + 0x10))} d0 {r(vec(b, sweep + 0x20))} '
            f'e0 {r(vec(b, sweep + 0x30))} {bits(b, sweep + 0x30)}\n'
            f'  part {r(vec(b, partp))} {bits(b, partp)} race.60 {r(vec(b, r60))} race.120 {r(vec(b, r120))} '
            f'{bits(b, r120)}\n')
        out.flush()
    th.join()
    out.write(f'# torn {torn}\n')
    out.close()
    print(f'wrote {a.out}, torn {torn}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
