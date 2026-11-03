"""S252: capture a free06 HasPhysics truck woken by the biplane's search, saving states around it.

usage: python3 -B research/s252/strike.py NAME SLOT TRUCK [--frames N] [--late N] [--near FT]

watch/s252-truck.toml, one row a tick, from save state SLOT (research/s252/aim.py's: the plane
parked in truck TRUCK's way, 0 ctruck, 1 the other) into context/capture/s252/NAME.csv. Save
states as research/s241/strike.py's: slots 3, 8, 9 and 0 the first four ticks the plane's body is
within --near ft of the truck, slots 4, 5 and 6 the first three ticks the truck is physical (+0x588,
the struck listener's wake and the two after), slot 7 --late ticks after. Each is copied to
context/s252/NAME-<frame asked>.p2s at the end.

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
from ps2kit.config import cfg            # noqa: E402

TRUCKS = [0x01cc4440, 0x01cc7630]
PLANE = 0x00c44a40


class Wreck(pine_mod.Pine):
    def __init__(self, body: int, near: float, late: int):
        super().__init__(check=True)
        self.body, self.near, self.late = body, near, late
        self.saved: list[tuple[int, int]] = []
        self.pre = False
        self.woke_at: int | None = None
        self.wake_slots = [4, 5, 6]
        self.pre_slots = [3, 8, 9, 0]

    def _plane(self) -> int:
        """The plane's body (MXBiplane +0x160)."""
        return self.read32(PLANE + 0x160)

    def _save(self, v: int, slot: int, what: str):
        self.save_state(slot)
        self.saved.append((v, slot))
        print(f'  frame {v}: {what}, saving into slot {slot}', flush=True)

    def next_frame(self, last, *a, **k):
        v = super().next_frame(last, *a, **k)
        truck = struct.unpack('<3f', self.read_block(self.body + 0x470, 12))
        if self.pre_slots:
            c = struct.unpack('<3f', self.read_block(self._plane() + 0x60, 12))
            if self.pre or math.dist(c, truck) < self.near:
                self.pre = True
                what = 'the plane %.1f ft off the truck' % math.dist(c, truck)
                self._save(v, self.pre_slots.pop(0), what)
        if self.woke_at is None and self.read32(self.body + 0x588):
            self.woke_at = v
            print(f'the truck woke at frame {v}', flush=True)
        if self.woke_at is not None and self.wake_slots:
            self._save(v, self.wake_slots.pop(0), 'woken')
        if self.woke_at is not None and self.late is not None and v >= self.woke_at + self.late:
            self._save(v, 7, 'late')
            self.late = None
        return v


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument('name')
    ap.add_argument('slot', type=int)
    ap.add_argument('truck', type=int)
    ap.add_argument('--frames', type=int, default=240)
    ap.add_argument('--late', type=int, default=15)
    ap.add_argument('--near', type=float, default=30.0)
    a = ap.parse_args()
    out = ROOT / 'context' / 'capture' / 's252'
    out.mkdir(parents=True, exist_ok=True)
    keep = ROOT / 'context' / 's252'
    keep.mkdir(parents=True, exist_ok=True)
    p = Wreck(TRUCKS[a.truck], a.near, a.late)
    watch = record.Watch(ROOT / 'watch' / 's252-truck.toml')
    res = record.record(watch, out / f'{a.name}.csv', frames=a.frames, state=a.slot, pine=p)
    print(f'{res.frames} rows, frames {res.first}..{res.last}, {res.skipped} torn, {res.dropped} dropped; '
          f'ended: {res.ended}')
    time.sleep(2.0)                                   # let the last save finish writing
    for v, slot in p.saved:
        dst = keep / f'{a.name}-{v}.p2s'
        shutil.copyfile(emu.state_path(str(slot)), dst)
        print(f'  slot {slot} (asked at frame {v}) -> {dst.relative_to(ROOT)}')
    if p.woke_at is None:
        print('the truck never woke')
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
