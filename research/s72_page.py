"""S72: capture the wreck page — the ride state's request and the viewport fader that puts the bike back.

usage: python3 -B research/s72_page.py NAME --to X,Z [--state SLOT] [--frames N] [--after N]

S59's edge ride (research/s59_edge.py: gas held, the stick steering toward X,Z over PINE until the
cannonball, then `--after` frames hands off), recorded with watch/s72-page.toml into
context/capture/s72/NAME.csv. The cannonball is the one wreck that is sure to happen unattended, and
every wreck but the lost driver's asks for the page.

Run it under `./kit pcsx2 with --`, with `./kit pad serve` up for the copy.
"""

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
sys.path.insert(0, str(Path(__file__).resolve().parent))
from ps2kit import pad                   # noqa: E402
from ps2kit import record                # noqa: E402
from s59_edge import Steering            # noqa: E402

ROOT = Path(__file__).resolve().parent.parent


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument('name')
    ap.add_argument('--to', required=True, help='X,Z to steer toward')
    ap.add_argument('--state', type=int, default=7)
    ap.add_argument('--frames', type=int, default=3000)
    ap.add_argument('--after', type=int, default=600)
    a = ap.parse_args()
    to = tuple(float(x) for x in a.to.split(','))
    out = ROOT / 'context' / 'capture' / 's72'
    out.mkdir(parents=True, exist_ok=True)
    p = Steering(to, a.after)
    watch = record.Watch(ROOT / 'watch' / 's72-page.toml')
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
