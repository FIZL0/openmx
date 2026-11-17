"""F21: follow free11's bowling pins tick by tick from a save state, and save states round a meeting.

usage: python3 -B research/f21/strike.py NAME SLOT [--frames N] [--at F,F,...]

From save state SLOT (research/f21/toss.py's: one pin awake, tossed at another), one line a tick: the
frame, the props node's filed count, and each awake pin's centre and its nearest sibling. With --at,
states are saved (research/s242/strike.py's way, from this PINE client) into slots 3..8 on the ticks
of those frames, and copied to context/f21/NAME-<frame>.p2s at the end.

Run it under `KIT_PCSX2=N ./kit pcsx2 with --`, the game running.
"""

import argparse
import math
import shutil
import struct
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(ROOT))
from ps2kit import emu                   # noqa: E402
from ps2kit.pine import Pine             # noqa: E402

BODIES = [0xd0add0, 0xd0b3f0, 0xd0fe60, 0xd12f20, 0xd15fe0, 0xd190a0, 0xd1c180, 0xd1f240, 0xd22300, 0xd253c0]
NODE = 0x007a3d40
FRAME = 0x00479d04


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument('name')
    ap.add_argument('slot', type=int)
    ap.add_argument('--frames', type=int, default=40)
    ap.add_argument('--at', default='')
    a = ap.parse_args()
    at = [int(f) for f in a.at.split(',') if f]
    slots = [3, 4, 5, 6, 7, 8]
    saved = []
    p = Pine()
    p.load_state(a.slot)
    time.sleep(0.5)
    last = p.read32(FRAME)
    for _ in range(a.frames):
        f = p.next_frame(last, addr=FRAME)
        if f != last + 1:
            print(f'  ({f - last - 1} frames missed)')
        last = f
        if f in at and slots:
            s = slots.pop(0)
            p.save_state(s)
            saved.append((f, s))
        pos = [struct.unpack('<3f', p.read_block(b + 0x60, 12)) for b in BODIES]
        on = [p.read32(b) for b in BODIES]
        line = []
        for k in range(10):
            if on[k]:
                d, j = min((math.dist(pos[k], pos[j]), j) for j in range(10) if j != k)
                line.append('pin%d (%.2f %.2f %.2f) near %d %.2f' % (k, *pos[k], j, d))
        print(f, 'filed', p.read32(NODE + 0x30), *line, flush=True)
    time.sleep(2.0)
    keep = ROOT / 'context' / 'f21'
    keep.mkdir(parents=True, exist_ok=True)
    for f, s in saved:
        shutil.copyfile(emu.state_path(str(s)), keep / f'{a.name}-{f}.p2s')
        print(f'  slot {s} (frame {f}) -> context/f21/{a.name}-{f}.p2s')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
