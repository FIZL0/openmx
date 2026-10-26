"""S84: capture the L1+R1 chord inside the wreck page's fade out — where the fader's second respawn puts the bike.

usage: python3 -B research/s84_chord.py NAME [--state SLOT] [--delay N] [--after N] [--frames N]

`--state` (default 9: free09's state 7 as S72 used it, which starts in its own reason-6 wreck, 0.2 s
in) is loaded and recorded with watch/s84-chord.toml into context/capture/s84/NAME.csv. L1 is held
from the first row; on the `--delay`th row (default 0) after one shows a viewport fader fading out
(the page's request tick, or the one after on a late read) R1 goes down too, which is the chord's
press event, then both come up 10 rows later. The capture stops `--after` rows (default 150) past
the press: the chord's put-back, the fader's black draw and its fade back in, with the ghost.

The press is wall-clock (`./kit pad`), so which tick it lands on is read off the capture (the chord
latch +0x14d0 and pad 0's buffer), not assumed.

Run it under `./kit pcsx2 with --`, with `./kit pad serve` up for the copy.
"""

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from ps2kit import pad                   # noqa: E402
from ps2kit import pine as pine_mod      # noqa: E402
from ps2kit import record                # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
FADERS = (0x41f770, 0x3d0, 0xd4)         # the video fx manager's viewport faders, +0x40 and +0x44
FADING_OUT = 0x48


class Chord(pine_mod.Pine):
    """The recorder's PINE client: L1 held, R1 pressed when a fader starts fading out."""

    def __init__(self, delay: int, after: int):
        super().__init__(check=True)
        self.delay, self.after = delay, after
        self.seen_at: int | None = None
        self.pressed_at: int | None = None
        self.started = False

    def _fading_out(self) -> bool:
        p = self.read32(FADERS[0])
        for off in FADERS[1:]:
            if not 0 < p < 0x0200_0000:
                return False
            p = self.read32(p + off)
            if not 0 < p < 0x0200_0000:
                return False
        for f in (0x40, 0x44):
            q = self.read32(p + f)
            if 0 < q < 0x0200_0000 and self.read32(q + FADING_OUT):
                return True
        return False

    def next_frame(self, last, *a, **k):
        v = super().next_frame(last, *a, **k)
        if not self.started:
            pad.send(['release', 'down l1'], quiet=True)
            self.started = True
            return v
        if self.pressed_at is not None:
            if v - self.pressed_at == 10:
                pad.send(['up r1', 'up l1'], quiet=True)
            if v - self.pressed_at >= self.after:
                raise KeyboardInterrupt          # the recorder's own stop: it keeps the rows
            return v
        if self.seen_at is None and self._fading_out():
            self.seen_at = v
            print(f'fading out at frame {v}', flush=True)
        if self.seen_at is not None and v - self.seen_at >= self.delay:
            pad.send(['down r1'], quiet=True)
            self.pressed_at = v
            print(f'R1 down at frame {v}', flush=True)
        return v


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument('name')
    ap.add_argument('--state', type=int, default=9)
    ap.add_argument('--delay', type=int, default=0)
    ap.add_argument('--after', type=int, default=150)
    ap.add_argument('--frames', type=int, default=600)
    a = ap.parse_args()
    out = ROOT / 'context' / 'capture' / 's84'
    out.mkdir(parents=True, exist_ok=True)
    p = Chord(a.delay, a.after)
    watch = record.Watch(ROOT / 'watch' / 's84-chord.toml')
    try:
        res = record.record(watch, out / f'{a.name}.csv', frames=a.frames, state=a.state, pine=p)
    finally:
        pad.send(['release'], quiet=True)
    print(f'{res.frames} rows, frames {res.first}..{res.last}, {res.skipped} torn, {res.dropped} dropped; '
          f'ended: {res.ended}')
    if p.pressed_at is None:
        print('no page')
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
