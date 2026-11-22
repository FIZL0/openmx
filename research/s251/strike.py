"""S251: record S207's ctruck ride again (s89-along, the bike parked in the company truck's way by
research/s207/park.py) and save states around the strike, for the interpreter's wake.

usage: python3 -B research/s251/strike.py NAME SLOT [--frames N] [--near FT]

Records watch/s129-train.toml (S207's spec for ctruck), one row a tick, from save state SLOT into
context/capture/s251/NAME.csv. Save states, as research/s252a/strike.py's: a ring of slots 3, 8, 9, 0,
10, 11 every tick the bike's vehicle (0x00a00050 +0x1b0) is within --near ft of the truck's path point
(+0x470), slots 4, 5 and 6 the first three ticks the truck (0x01cc7630) is physical (+0x588). The last
four ring saves before the wake and the three wake saves are copied to context/s251/NAME-<frame asked>.p2s
at the end.

Run it under `KIT_PCSX2=N ./kit pcsx2 with --`, the game running (not paused).
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

TRUCK = 0x01cc7630
VEHICLE = 0x00a00050


class Strike(pine_mod.Pine):
    def __init__(self, near: float):
        super().__init__(check=True)
        self.near = near
        self.ring = [3, 8, 9, 0, 10, 11]
        self.turn = 0
        self.pre: list[tuple[int, int]] = []
        self.woken: list[tuple[int, int]] = []
        self.woke_at: int | None = None
        self.wake_slots = [4, 5, 6]

    def next_frame(self, last, *a, **k):
        v = super().next_frame(last, *a, **k)
        truck = struct.unpack('<3f', self.read_block(TRUCK + 0x470, 12))
        bike = struct.unpack('<3f', self.read_block(VEHICLE + 0x1b0, 12))
        if self.woke_at is None and self.read32(TRUCK + 0x588):
            self.woke_at = v
            print(f'the truck woke at frame {v}', flush=True)
        if self.woke_at is None:
            d = math.dist(truck, bike)
            if d < self.near:
                slot = self.ring[self.turn % len(self.ring)]
                self.turn += 1
                self.save_state(slot)
                self.pre = [(f, s) for f, s in self.pre if s != slot] + [(v, slot)]
                print(f'  frame {v}: the bike {d:.1f} ft off, saving into slot {slot}', flush=True)
        elif self.wake_slots:
            slot = self.wake_slots.pop(0)
            self.save_state(slot)
            self.woken.append((v, slot))
            print(f'  frame {v}: woken, saving into slot {slot}', flush=True)
        return v


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument('name')
    ap.add_argument('slot', type=int)
    ap.add_argument('--frames', type=int, default=300)
    ap.add_argument('--near', type=float, default=20.0)
    a = ap.parse_args()
    out = ROOT / 'context' / 'capture' / 's251'
    out.mkdir(parents=True, exist_ok=True)
    keep = ROOT / 'context' / 's251'
    keep.mkdir(parents=True, exist_ok=True)
    p = Strike(a.near)
    watch = record.Watch(ROOT / 'watch' / 's129-train.toml')
    res = record.record(watch, out / f'{a.name}.csv', frames=a.frames, state=a.slot, pine=p)
    print(f'{res.frames} rows, frames {res.first}..{res.last}, {res.skipped} torn, {res.dropped} dropped; '
          f'ended: {res.ended}')
    time.sleep(3.0)                                   # let the last save finish writing
    for v, slot in p.pre[-4:] + p.woken:
        dst = keep / f'{a.name}-{v}.p2s'
        shutil.copyfile(emu.state_path(str(slot)), dst)
        print(f'  slot {slot} (asked at frame {v}) -> {dst.relative_to(ROOT)}')
    if p.woke_at is None:
        print('the truck never woke')
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
