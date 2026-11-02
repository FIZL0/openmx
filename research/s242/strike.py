"""S242: capture one free09 beach ball striking another, saving states round the strike.

usage: python3 -B research/s242/strike.py NAME SLOT BALL ONTO [--frames N] [--near FT] [--at F,F,...]

watch/s197b-wreck.toml (every ball's body, object and record), one row a tick, from save state SLOT
(research/s242/put_ball.py's: ball BALL put over ball ONTO) into context/capture/s242/NAME.csv. Save
states (research/s240/strike.py's way, from the recorder's PINE client) into slots 3..8 the first six
ticks the two bodies' centres are within --near ft (default 14), or with --at, the ticks of those
frames. Each is copied to context/s242/NAME-<frame asked>.p2s at the end.

Run it under `KIT_PCSX2=N ./kit pcsx2 with --`, with the game running.
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


class Strike(pine_mod.Pine):
    def __init__(self, a: int, b: int, near: float, at: list[int]):
        super().__init__(check=True)
        self.a, self.b, self.near, self.at = a, b, near, at
        self.saved: list[tuple[int, int]] = []
        self.slots = [3, 4, 5, 6, 7, 8]
        self.pre = False

    def next_frame(self, last, *a, **k):
        v = super().next_frame(last, *a, **k)
        if self.slots:
            pa = struct.unpack('<3f', self.read_block(self.a + 0x60, 12))
            pb = struct.unpack('<3f', self.read_block(self.b + 0x60, 12))
            d = math.dist(pa, pb)
            if v in self.at if self.at else self.pre or d < self.near:
                self.pre = True
                slot = self.slots.pop(0)
                self.save_state(slot)
                self.saved.append((v, slot))
                print(f'  frame {v}: the balls {d:.2f} ft apart, saving into slot {slot}', flush=True)
        return v


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument('name')
    ap.add_argument('slot', type=int)
    ap.add_argument('ball', type=int)
    ap.add_argument('onto', type=int)
    ap.add_argument('--frames', type=int, default=60)
    ap.add_argument('--near', type=float, default=14.0)
    ap.add_argument('--at', default='')
    a = ap.parse_args()
    out = ROOT / 'context' / 'capture' / 's242'
    out.mkdir(parents=True, exist_ok=True)
    keep = ROOT / 'context' / 's242'
    keep.mkdir(parents=True, exist_ok=True)
    p = Strike(BALLS[a.ball], BALLS[a.onto], a.near, [int(f) for f in a.at.split(',') if f])
    watch = record.Watch(ROOT / 'watch' / 's197b-wreck.toml')
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
