"""S197b: capture a free09 beach ball struck by the wreck bike, saving states around the strike.

usage: python3 -B research/s197b/wreck.py NAME SLOT BALL [--frames N] [--late N] [--near FT]

watch/s197b-wreck.toml, one row a tick, from save state SLOT (research/s197b/drop.py's: the bike
dropped hard beside ball BALL, 0..4 the five BeachBall collections in the list's order) into
context/capture/s197b/NAME.csv. Save states (research/s195/knock.py's way, from the recorder's PINE
client): slots 3, 8, 9 and 0 the first four ticks the rider is down (wreck reason set) and the wreck
bike's body within --near ft of the ball, slots 4, 5 and 6 the first three ticks the ball's body is
enabled (the strike's wake and the two after), slot 7 --late ticks after the wake. Each is copied to
context/s197b/NAME-<frame asked>.p2s at the end.

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

BODIES = [0x11b6030, 0x11b6a90, 0x11bf460, 0x11c1f50, 0x11c4a40]
RIDER = (0x34, 0x34, 0x8, 0x8, 0x0)
WRECK_BODY = 0x00dc8980


class Wreck(pine_mod.Pine):
    def __init__(self, body: int, near: float, late: int):
        super().__init__(check=True)
        self.body, self.near, self.late = body, near, late
        self.saved: list[tuple[int, int]] = []
        self.pre = False
        self.woke_at: int | None = None
        self.wake_slots = [4, 5, 6]
        self.pre_slots = [3, 8, 9, 0]

    def _rider(self) -> int:
        p = self.read32(cfg().symbols['game_manager'])
        for off in RIDER:
            if not 0 < p < 0x0200_0000:
                return 0
            p = self.read32(p + off)
        return p if 0 < p < 0x0200_0000 else 0

    def _wreck_bike(self) -> int:
        """The wreck bike's body once the rider is down: at 0x00dc8980 in every state made from
        first_recording's (watch/s197b-wreck.toml)."""
        r = self._rider()
        return WRECK_BODY if r and self.read32(r + 0x748) and self.read32(WRECK_BODY) else 0

    def _save(self, v: int, slot: int, what: str):
        self.save_state(slot)
        self.saved.append((v, slot))
        print(f'  frame {v}: {what}, saving into slot {slot}', flush=True)

    def next_frame(self, last, *a, **k):
        v = super().next_frame(last, *a, **k)
        ball = struct.unpack('<3f', self.read_block(self.body + 0x60, 12))
        if self.pre_slots:
            wb = self._wreck_bike()
            c = struct.unpack('<3f', self.read_block(wb + 0x60, 12)) if wb else None
            if self.pre or c and math.dist(c, ball) < self.near:
                self.pre = True
                what = 'the wreck bike %.1f ft off the ball' % math.dist(c, ball) if c else 'near'
                self._save(v, self.pre_slots.pop(0), what)
        if self.woke_at is None and self.read32(self.body):
            self.woke_at = v
            print(f'the ball woke at frame {v}', flush=True)
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
    ap.add_argument('ball', type=int)
    ap.add_argument('--frames', type=int, default=240)
    ap.add_argument('--late', type=int, default=15)
    ap.add_argument('--near', type=float, default=8.0)
    a = ap.parse_args()
    out = ROOT / 'context' / 'capture' / 's197b'
    out.mkdir(parents=True, exist_ok=True)
    keep = ROOT / 'context' / 's197b'
    keep.mkdir(parents=True, exist_ok=True)
    p = Wreck(BODIES[a.ball], a.near, a.late)
    watch = record.Watch(ROOT / 'watch' / 's197b-wreck.toml')
    res = record.record(watch, out / f'{a.name}.csv', frames=a.frames, state=a.slot, pine=p)
    print(f'{res.frames} rows, frames {res.first}..{res.last}, {res.skipped} torn, {res.dropped} dropped; '
          f'ended: {res.ended}')
    time.sleep(2.0)                                   # let the last save finish writing
    for v, slot in p.saved:
        dst = keep / f'{a.name}-{v}.p2s'
        shutil.copyfile(emu.state_path(str(slot)), dst)
        print(f'  slot {slot} (asked at frame {v}) -> {dst.relative_to(ROOT)}')
    if p.woke_at is None:
        print('the ball never woke')
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
