"""S59: capture the original's map edge — the human's bike ridden off the map, thrown back, put back.

usage: python3 -B research/s59_edge.py NAME --to X,Z [--state SLOT] [--frames N] [--after N]
                                        [--kick FT/S [--kick-at FEET]]

Runs watch/s59-edge.toml through `ps2kit.record` into context/capture/s59/NAME.csv. `--state`
(default 7) is loaded first; then gas is held and the stick steers, closed-loop over PINE, the
chassis' velocity toward the point X,Z (feet, the map's own coordinates: past an edge to ride off
it). Pushing the stick right turns the velocity's atan2(z, x) down, measured on free09.

From the first tick the rider is down for the cannonball (reason 13) the stick and gas are let go
and the capture runs `--after` more frames (default 600: the 2.5 s settle, the 5 s wreck time and
the put-back, at 60 frames a second), then stops.

`--kick` makes the crossing one in the air: within `--kick-at` feet of the margin the chassis'
upward velocity is written once over PINE (free09's edges are a flat strip ~480 ft deep behind a
berm, and neither stick pulled back and pushed makes the bike leave it: journal 2026-10-21-s59).

Run it under `./kit pcsx2 with --`, with `./kit pad serve` up for the copy.
"""

import argparse
import math
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from ps2kit import pad                   # noqa: E402
from ps2kit import pine as pine_mod      # noqa: E402
from ps2kit import record                # noqa: E402
from ps2kit.config import cfg            # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
RIDER = (0x34, 0x34, 0x8, 0x8, 0x0)      # @game_manager's chain to the human's rider record
REASON, CHASSIS = 0x748, 0x7a4           # its wreck reason, its chassis
COFG, VEL = 0x60, 0x260                  # the chassis' centre of gravity, velocity
CANNONBALL = 13
GAIN = 1.0 / 30.0                        # stick per degree off course


class Steering(pine_mod.Pine):
    """The recorder's PINE client, steering on every third frame until the cannonball."""

    def __init__(self, to: tuple[float, float], after: int, kick: float = 0.0, kick_at: float = 0.0):
        super().__init__(check=True)
        self.to, self.after = to, after
        self.kick, self.kick_at = kick, kick_at
        self.kicked_at: int | None = None
        self.thrown_at: int | None = None
        self.n = 0
        self.started = False

    def _bike(self) -> int:
        p = self.read32(cfg().symbols['game_manager'])
        for off in RIDER:
            if not 0 < p < 0x0200_0000:
                return 0
            p = self.read32(p + off)
        return p if 0 < p < 0x0200_0000 else 0

    def _v(self, a: int) -> tuple[float, ...]:
        return struct.unpack('<4f', self.read_block(a, 16))

    def next_frame(self, last, *a, **k):
        v = super().next_frame(last, *a, **k)
        self.n += 1
        if self.thrown_at is not None:
            if v - self.thrown_at >= self.after:
                raise KeyboardInterrupt          # the recorder's own stop: it keeps the rows
            return v
        b = self._bike()
        if not b:
            return v
        if self.read32(b + REASON) == CANNONBALL:
            self.thrown_at = v
            pad.send(['release'], quiet=True)
            print(f'cannonball at frame {v}', flush=True)
            return v
        if not self.started:
            pad.send(['release', 'down cross'], quiet=True)
            self.started = True
        if self.n % 3:
            return v
        ch = self.read32(b + CHASSIS)
        c, vel = self._v(ch + COFG), self._v(ch + VEL)
        want = math.degrees(math.atan2(self.to[1] - c[2], self.to[0] - c[0]))
        have = math.degrees(math.atan2(vel[2], vel[0]))
        err = (want - have + 180.0) % 360.0 - 180.0
        if math.hypot(vel[0], vel[2]) < 5.0:
            err = 0.0                            # no heading to steer yet
        sx = max(-1.0, min(1.0, -err * GAIN))
        # The kick: within `kick_at` feet of the margin the chassis' upward velocity is written once,
        # and the original's own physics carries the bike over the margin in the air.
        edge = min(c[0] - 150.0, 6762.0 - c[0], c[2] - 150.0, 6762.0 - c[2])
        if self.kick and self.kicked_at is None and edge < self.kick_at:
            self.write_f32(ch + VEL + 4, self.kick)
            self.kicked_at = v
            print(f'kicked {self.kick} ft/s up at frame {v}, {edge:.0f} ft in', flush=True)
        pad.send([f'stick l {sx:.2f} 0'], quiet=True)
        return v


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument('name')
    ap.add_argument('--to', required=True, help='X,Z to steer toward')
    ap.add_argument('--state', type=int, default=7)
    ap.add_argument('--frames', type=int, default=3000)
    ap.add_argument('--after', type=int, default=600)
    ap.add_argument('--kick', type=float, default=0.0, help='upward velocity to write once, ft/s')
    ap.add_argument('--kick-at', type=float, default=130.0, help='feet from the margin to write it at')
    a = ap.parse_args()
    to = tuple(float(x) for x in a.to.split(','))
    out = ROOT / 'context' / 'capture' / 's59'
    out.mkdir(parents=True, exist_ok=True)
    p = Steering(to, a.after, a.kick, a.kick_at)
    watch = record.Watch(ROOT / 'watch' / 's59-edge.toml')
    try:
        res = record.record(watch, out / f'{a.name}.csv', frames=a.frames, state=a.state, pine=p)
    finally:
        pad.send(['release'], quiet=True)
    print(f'{res.frames} rows, frames {res.first}..{res.last}, {res.skipped} torn, {res.dropped} dropped; '
          f'ended: {res.ended}')
    if p.thrown_at is None:
        print('no cannonball')
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
