#!/usr/bin/env python3
r"""S77: where the original's put-back record goes during and after a cannonball (the map edge's throw).

    ./kit pcsx2 with -- python3 -B research/s77_edge_back.py --state 7 --to 7000,3400 \
        --out context/s77/free09-east.txt

Loads the state, holds gas and steers the human's bike (closed loop over PINE, as research/s59_edge.py
does) toward X,Z past an edge until the edge clause throws it (reason 13). Then the pad is let go; the
wreck clock puts the bike back. After the put-back gas is held again for `--ride` ticks, then L1+R1 is
pressed once (`--chord`, 0 = never) and the capture runs `--tail` more frames.

One block a frame, every float with its bits:
  +0x748 reason, +0x868 its copy, +0xc84 the request's copy (put_back_keep's 13 test), +0x794 clock
  +0x3b0/+0x3c0/+0x3d0 the put-back record (point, flat heading, up)
  node (+0x18) world rows +0xa0 (row 2) and +0xb0 (the translation): the cannonball branch's probe;
  chassis +0x254 is checked to be the same node
  *(+0x7a8)+0x120 -> +0x40..+0x4c: the wreck bike's collision record's touched / other / ground / object
  *(+0x7a8)+0x124 -> +0x40: the part point the overhead probe stands on while down
  *(+0x7a8)+0x110: the wreck body (+0x50 position, +0x60, +0x70 velocity)
  chassis +0x60 the cofg; collision record (+0x7d0) +0xe0 the riding probe; race +0x120 the heading
Research, not product.
"""

import argparse
import math
import os
import struct
import sys
import time

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), '..'))
sys.path.insert(0, ROOT)

from ps2kit import pad  # noqa: E402
from ps2kit.pine import Pine  # noqa: E402

GAME_MANAGER = 0x004E1E98
FRAME_COUNTER = 0x00479D04
FRAME_DT = 0x00479CD4
CANNONBALL = 13
GAIN = 1.0 / 30.0


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


def vec(b, off, n=4):
    return [struct.unpack_from('<f', b, off + 4 * i)[0] for i in range(n)]


def bits(b, off, n=4):
    return ' '.join(f'{struct.unpack_from("<I", b, off + 4 * i)[0]:08x}' for i in range(n))


def u32(b, off):
    return struct.unpack_from('<I', b, off)[0]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument('--state', type=int, required=True)
    ap.add_argument('--to', required=True, help='X,Z to steer toward (past an edge)')
    ap.add_argument('--ride', type=int, default=120, help='frames of gas after the put-back')
    ap.add_argument('--chord', type=int, default=1, help='press L1+R1 after the ride (0: never)')
    ap.add_argument('--tail', type=int, default=240, help='frames after the chord')
    ap.add_argument('--limit', type=float, default=150.0, help='seconds before giving up')
    ap.add_argument('--out', required=True)
    a = ap.parse_args()
    to = tuple(float(x) for x in a.to.split(','))
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
    coll = p.read32(bike + 0x7d0)
    race = p.read32(bike + 0xf9c)
    rag = p.read32(bike + 0x7a8)
    rec = p.read32(rag + 0x120)
    part = p.read32(rag + 0x124)
    wbody = p.read32(rag + 0x110)
    out.write(f'# bike {bike:08x} node {node:08x} chassis {body:08x} chassis+0x254 {p.read32(body + 0x254):08x} '
              f'rag {rag:08x} rec {rec:08x} part {part:08x} wbody {wbody:08x} '
              f'+0x13a0 {p.read_f32(bike + 0x13a0)!r} armed {p.read32(bike + 0x570)}\n')
    regions = [
        (FRAME_COUNTER, 4), (FRAME_DT, 4),
        (bike + 0x3b0, 0x30), (bike + 0x748, 4), (bike + 0x794, 4), (bike + 0x868, 4), (bike + 0xc84, 4),
        (node + 0xa0, 0x20), (rec + 0x40, 0x10), (part + 0x40, 0x10), (wbody + 0x50, 0x30),
        (body + 0x60, 0x10), (coll + 0xe0, 0x10), (race + 0x120, 0x10),
    ]
    pad.send(['release', 'down cross'], quiet=True)
    prev = None
    n = 0
    torn = 0
    thrown = back = chorded = None
    phase = 'steer'
    while time.time() - t0 < a.limit:
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
        n += 1
        o = 8
        r_rec = o; o += 0x30
        reason = u32(b, o); o += 4
        clock = vec(b, o, 1)[0]; o += 4
        r868 = u32(b, o); o += 4
        c84 = u32(b, o); o += 4
        r_node = o; o += 0x20
        r_flag = o; o += 0x10
        r_part = o; o += 0x10
        r_wb = o; o += 0x30
        r_cofg = o; o += 0x10
        r_sweep = o; o += 0x10
        r_head = o
        fl = [u32(b, r_flag + 4 * i) for i in range(4)]
        r = lambda v: '[' + ', '.join(f'{x:.4f}' for x in v) + ']'  # noqa: E731
        out.write(
            f'f {u32(b, 0)} dt {vec(b, 4, 1)[0]:.6f} phase {phase} reason {reason} r868 {r868} c84 {c84} '
            f'clk {clock:.4f} touch {fl}\n'
            f'  rec.p {r(vec(b, r_rec))} {bits(b, r_rec)}\n'
            f'  rec.f {r(vec(b, r_rec + 0x10))} {bits(b, r_rec + 0x10)}\n'
            f'  rec.u {r(vec(b, r_rec + 0x20))} {bits(b, r_rec + 0x20)}\n'
            f'  node.r2 {r(vec(b, r_node))} {bits(b, r_node)}\n'
            f'  node.t {r(vec(b, r_node + 0x10))} {bits(b, r_node + 0x10)}\n'
            f'  part {r(vec(b, r_part))} {bits(b, r_part)}\n'
            f'  wb.50 {r(vec(b, r_wb))} 60 {r(vec(b, r_wb + 0x10))} 70 {r(vec(b, r_wb + 0x20))}\n'
            f'  cofg {r(vec(b, r_cofg))} sweep {r(vec(b, r_sweep))} {bits(b, r_sweep)} '
            f'head {r(vec(b, r_head))} {bits(b, r_head)}\n')
        out.flush()

        if phase == 'steer':
            if reason == CANNONBALL:
                thrown = f
                phase = 'thrown'
                pad.send(['release'], quiet=True)
                print(f'cannonball at frame {f}', flush=True)
            elif n % 3 == 0:
                c, vel = vec(b, r_cofg), vec(b, r_wb + 0x20)
                ch_vel = struct.unpack('<4f', p.read_block(body + 0x260, 16))
                want = math.degrees(math.atan2(to[1] - c[2], to[0] - c[0]))
                have = math.degrees(math.atan2(ch_vel[2], ch_vel[0]))
                err = (want - have + 180.0) % 360.0 - 180.0
                if math.hypot(ch_vel[0], ch_vel[2]) < 5.0:
                    err = 0.0
                sx = max(-1.0, min(1.0, -err * GAIN))
                pad.send([f'stick l {sx:.2f} 0'], quiet=True)
        elif phase == 'thrown':
            if reason == 0:
                back = f
                phase = 'ride'
                pad.send(['release', 'down cross'], quiet=True)
                print(f'put back at frame {f}', flush=True)
        elif phase == 'ride':
            if f - back >= a.ride:
                if a.chord:
                    pad.send(['up cross', 'down l1'], quiet=True)
                    time.sleep(0.12)
                    pad.send(['down r1'], quiet=True)
                    time.sleep(0.2)
                    pad.send(['up l1', 'up r1'], quiet=True)
                    print(f'chord at frame {f}', flush=True)
                chorded = f
                phase = 'tail'
        elif phase == 'tail':
            if f - chorded >= a.tail:
                break
    pad.send(['release'], quiet=True)
    out.write(f'# torn {torn} thrown {thrown} back {back} chord {chorded}\n')
    out.close()
    print(f'wrote {a.out}: {n} rows, torn {torn}, thrown {thrown}, back {back}, chord {chorded}')
    return 0 if back is not None else 1


if __name__ == '__main__':
    raise SystemExit(main())
