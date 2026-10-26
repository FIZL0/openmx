"""S138: capture the original's bike into a wall or onto its front wheel, with save states before the hit.

usage: python3 -B research/s138_hits.py NAME --pad SCRIPT --state SLOT --when EXPR [--frames N] [--save 5,6,7]

Runs research/s138/hits.toml (S67's whole-bike spec on recording2's rider chain, wreck reason included)
through `ps2kit.record` into context/capture/s138/NAME.csv, from save state `--state` with the pad
script played from there. From the first tick EXPR holds — a Python expression over the chassis' centre
of mass `x y z` and velocity `vx vy vz` (+0x60, +0x260) — a state is saved into each `--save` slot in
turn, one a tick: what a test starts the port from a few ticks before the impact. Each is copied to
context/capture/s138/NAME-<frame asked>.p2s at the end. The wreck reason's first tick is printed.

Run it under `./kit pcsx2 with --`, with `./kit pad serve` up for the copy and the game not paused.
"""

import argparse
import shutil
import struct
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from ps2kit import emu                   # noqa: E402
from ps2kit import pine as pine_mod      # noqa: E402
from ps2kit import record                # noqa: E402
from ps2kit.config import cfg            # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
RIDER = (0x34, 0x34, 0x8, 0x8, 0x0)      # @game_manager's chain to the human's rider record
WRECK = 0x748                            # its wreck reason
CHASSIS = 0x7a4                          # its chassis


class When(pine_mod.Pine):
    def __init__(self, slots, expr):
        super().__init__(check=True)
        self.slots, self.expr = list(slots), compile(expr, '--when', 'eval')
        self.saved, self.armed, self.wreck_at, self.reason = [], False, None, 0

    def _rider(self):
        p = self.read32(cfg().symbols['game_manager'])
        for off in RIDER:
            if not 0 < p < 0x0200_0000:
                return 0
            p = self.read32(p + off)
        return p if 0 < p < 0x0200_0000 else 0

    def next_frame(self, last, *a, **k):
        v = super().next_frame(last, *a, **k)
        r = self._rider()
        if not r:
            return v
        if self.wreck_at is None:
            w = self.read32(r + WRECK)
            if w:
                self.wreck_at, self.reason = v, w
                print(f'wreck reason {w} at frame {v}', flush=True)
        if not self.armed:
            ch = self.read32(r + CHASSIS)
            x, y, z = struct.unpack('<3f', self.read_block(ch + 0x60, 12))
            vx, vy, vz = struct.unpack('<3f', self.read_block(ch + 0x260, 12))
            self.armed = bool(eval(self.expr, {}, dict(x=x, y=y, z=z, vx=vx, vy=vy, vz=vz)))
            if self.armed:
                print(f'frame {v}: --when holds at ({x:.2f}, {y:.2f}, {z:.2f}) moving ({vx:.2f}, {vy:.2f}, {vz:.2f})',
                      flush=True)
        if self.armed and self.slots:
            slot = self.slots.pop(0)
            self.save_state(slot)
            self.saved.append((v, slot))
            print(f'  frame {v}: saving a state into slot {slot}', flush=True)
        return v


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument('name')
    ap.add_argument('--pad', required=True)
    ap.add_argument('--state', type=int, required=True)
    ap.add_argument('--when', required=True)
    ap.add_argument('--save', default='5,6,7')
    ap.add_argument('--frames', type=int, default=300)
    a = ap.parse_args()
    out = ROOT / 'context' / 'capture' / 's138'
    out.mkdir(parents=True, exist_ok=True)
    p = When([int(s) for s in a.save.split(',')], a.when)
    watch = record.Watch(ROOT / 'research' / 's138' / 'hits.toml')
    res = record.record(watch, out / f'{a.name}.csv', frames=a.frames, state=a.state, pad=a.pad, pine=p)
    print(f'{res.frames} rows, frames {res.first}..{res.last}, {res.skipped} torn, {res.dropped} dropped; '
          f'ended: {res.ended}')
    print(f'wreck reason {p.reason} first read at frame {p.wreck_at}' if p.wreck_at else 'no wreck')
    if not p.saved:
        print('--when never held: no state saved')
        return 1
    time.sleep(2.0)                                   # let the last save finish writing
    for v, slot in p.saved:
        dst = out / f'{a.name}-{v}.p2s'
        shutil.copyfile(emu.state_path(str(slot)), dst)
        print(f'  slot {slot} (asked at frame {v}) -> {dst.relative_to(ROOT)}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
