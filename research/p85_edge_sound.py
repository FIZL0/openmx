"""P85: capture the cannonball's sounds and the music fade — S59's closed-loop ride off the map edge
(research/s59_edge.py) recorded through watch/p85-edge-sound.toml into context/capture/p85/NAME.csv.

usage: python3 -B research/p85_edge_sound.py NAME --to X,Z [--state SLOT] [--frames N] [--after N]

Run it under `./kit pcsx2 with --`, with `./kit pad serve` up for the copy.
"""

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
sys.path.insert(0, str(Path(__file__).resolve().parent))
from ps2kit import pad       # noqa: E402
from ps2kit import record    # noqa: E402
from s59_edge import Steering  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument('name')
    ap.add_argument('--to', required=True, help='X,Z to steer toward')
    ap.add_argument('--state', type=int, default=7)
    ap.add_argument('--frames', type=int, default=3000)
    ap.add_argument('--after', type=int, default=700)
    a = ap.parse_args()
    to = tuple(float(x) for x in a.to.split(','))
    out = ROOT / 'context' / 'capture' / 'p85'
    out.mkdir(parents=True, exist_ok=True)
    p = Steering(to, a.after)
    watch = record.Watch(ROOT / 'watch' / 'p85-edge-sound.toml')
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
