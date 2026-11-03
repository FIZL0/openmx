"""S252a: capture free06's HasPhysics truck woken by a downed bike, saving states around the wake.

usage: python3 -B research/s252a/strike.py NAME SLOT [--frames N] [--late N] [--near FT]

watch/s252a-downed.toml, one row a tick, from save state SLOT (research/s252a/drop.py's: the bike
wrecked on the line of the truck at 0x01cc7630) into context/capture/s252a/NAME.csv. Save states as
research/s252/strike.py's: slots 3, 8, 9 and 0 the first four ticks the wreck bike's body or a ragdoll
bone is within --near ft of the truck, slots 4, 5 and 6 the first three ticks the truck is physical
(+0x588: the struck listener's wake and the two after), slot 7 --late ticks after. Each is copied to
context/s252a/NAME-<frame asked>.p2s at the end.

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

TRUCK = 0x01cc7630
WRECK = 0x0196f020
ACTOR = 0x009ffc50


class Wreck(pine_mod.Pine):
    def __init__(self, near: float, late: int):
        super().__init__(check=True)
        self.near, self.late = near, late
        self.saved: list[tuple[int, int]] = []
        self.pre = False
        self.woke_at: int | None = None
        self.wake_slots = [4, 5, 6]
        self.pre_slots = [3, 8, 9, 0]

    def _near(self, truck) -> float:
        """The nearest of the wreck bike's body and the ragdoll's bones to the truck, ft."""
        at = [struct.unpack('<3f', self.read_block(WRECK + 0x60, 12))]
        parts = self.read32(ACTOR + 0x2f4)
        for i in range(18):
            at.append(struct.unpack('<3f', self.read_block(self.read32(parts + i * 0xb0 + 0xa4) + 0x60, 12)))
        return min(math.dist(c, truck) for c in at)

    def _save(self, v: int, slot: int, what: str):
        self.save_state(slot)
        self.saved.append((v, slot))
        print(f'  frame {v}: {what}, saving into slot {slot}', flush=True)

    def next_frame(self, last, *a, **k):
        v = super().next_frame(last, *a, **k)
        truck = struct.unpack('<3f', self.read_block(TRUCK + 0x470, 12))
        if self.pre_slots:
            d = self._near(truck)
            if self.pre or d < self.near:
                self.pre = True
                self._save(v, self.pre_slots.pop(0), 'the wreck %.1f ft off the truck' % d)
        if self.woke_at is None and self.read32(TRUCK + 0x588):
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
    ap.add_argument('--frames', type=int, default=240)
    ap.add_argument('--late', type=int, default=15)
    ap.add_argument('--near', type=float, default=15.0)
    a = ap.parse_args()
    out = ROOT / 'context' / 'capture' / 's252a'
    out.mkdir(parents=True, exist_ok=True)
    keep = ROOT / 'context' / 's252a'
    keep.mkdir(parents=True, exist_ok=True)
    p = Wreck(a.near, a.late)
    watch = record.Watch(ROOT / 'watch' / 's252a-downed.toml')
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
