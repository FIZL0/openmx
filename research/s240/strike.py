"""S240: capture a free09 beach ball struck by the thrown rider's ragdoll, saving states round the strike.

usage: python3 -B research/s240/strike.py NAME SLOT [--ball N] [--frames N] [--near FT] [--at F,F,...]

watch/s240-ragdoll.toml, one row a tick, from save state SLOT (research/s240/put_ball.py's: ball BALL,
default 1, put in the rider's way) into context/capture/s240/NAME.csv. Save states (research/s197b/wreck.py's
way, from the recorder's PINE client) into slots 3, 4, 5, 6, 7, 8 the first six ticks any bone's body is
within --near ft of the ball's (default 3), or with --at, the ticks of those frames. Each is copied to context/s240/NAME-<frame asked>.p2s at the end.

Run it under `KIT_PCSX2=N ./kit pcsx2 with --`, at a quarter speed, with the game running.
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
from ps2kit import pine as pine_mod      # noqa: E402
from ps2kit import record                # noqa: E402

BALLS = [0x11b6030, 0x11b6a90, 0x11bf460, 0x11c1f50, 0x11c4a40]
BONES = [0x17dc720, 0x17dcd80, 0x17dab00, 0x17daf00, 0x17db100, 0x17db700, 0x17dbf00, 0x17dc500, 0x17dcb60]


class Strike(pine_mod.Pine):
    def __init__(self, body: int, near: float, at: list[int]):
        super().__init__(check=True)
        self.body, self.near, self.at = body, near, at
        self.saved: list[tuple[int, int]] = []
        self.slots = [3, 4, 5, 6, 7, 8]
        self.pre = False

    def next_frame(self, last, *a, **k):
        v = super().next_frame(last, *a, **k)
        if self.slots:
            ball = struct.unpack('<3f', self.read_block(self.body + 0x60, 12))
            d = min(math.dist(struct.unpack('<3f', self.read_block(b + 0x60, 12)), ball) for b in BONES)
            if v in self.at if self.at else self.pre or d < self.near:
                self.pre = True
                slot = self.slots.pop(0)
                self.save_state(slot)
                self.saved.append((v, slot))
                print(f'  frame {v}: nearest bone {d:.2f} ft off the ball, saving into slot {slot}', flush=True)
        return v


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument('name')
    ap.add_argument('slot', type=int)
    ap.add_argument('--ball', type=int, default=1)
    ap.add_argument('--frames', type=int, default=90)
    ap.add_argument('--near', type=float, default=3.0)
    ap.add_argument('--at', default='')
    a = ap.parse_args()
    out = ROOT / 'context' / 'capture' / 's240'
    out.mkdir(parents=True, exist_ok=True)
    keep = ROOT / 'context' / 's240'
    keep.mkdir(parents=True, exist_ok=True)
    p = Strike(BALLS[a.ball], a.near, [int(f) for f in a.at.split(',') if f])
    watch = record.Watch(ROOT / 'watch' / 's240-ragdoll.toml')
    res = record.record(watch, out / f'{a.name}.csv', frames=a.frames, state=a.slot, pine=p)
    print(f'{res.frames} rows, frames {res.first}..{res.last}, {res.skipped} torn, {res.dropped} dropped; '
          f'ended: {res.ended}')
    time.sleep(2.0)                                   # let the last save finish writing
    for v, slot in p.saved:
        dst = keep / f'{a.name}-{v}.p2s'
        shutil.copyfile(emu.state_path(str(slot)), dst)
        print(f'  slot {slot} (asked at frame {v}) -> {dst.relative_to(ROOT)}')
    return 0 if p.saved else 1


if __name__ == '__main__':
    raise SystemExit(main())
