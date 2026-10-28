"""S134: capture the original wrecking into another rider's bike.

usage: python3 -B research/s134_wreck.py NAME --pad SCRIPT --state SLOT [--frames N] [--late N]

`research/s62_wreck.py`'s capture (save states the first three ticks of the wreck into slots 6, 7
and 8, and one `--late` ticks on into 9) through watch/s134-bike-hit.toml, which adds the three AI
riders' bikes to S56's spec, into context/capture/s134/NAME.csv; the states are copied to
context/capture/s134/NAME-<frame asked>.p2s at the end. Start from a state `research/s134_tele.py`
saved, under `./kit pcsx2 with --`, with `./kit pad serve` up and the game resumed.
"""

import argparse
import shutil
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
sys.path.insert(0, str(Path(__file__).resolve().parent))
from ps2kit import emu                   # noqa: E402
from ps2kit import record                # noqa: E402
from s62_wreck import Late               # noqa: E402

ROOT = Path(__file__).resolve().parent.parent


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument('name')
    ap.add_argument('--pad', required=True)
    ap.add_argument('--state', type=int, required=True)
    ap.add_argument('--frames', type=int, default=300)
    ap.add_argument('--late', type=int, default=120, help='ticks after the wreck for the last state')
    a = ap.parse_args()
    out = ROOT / 'context' / 'capture' / 's134'
    out.mkdir(parents=True, exist_ok=True)
    p = Late([6, 7, 8], a.late, 9)
    watch = record.Watch(ROOT / 'watch' / 's134-bike-hit.toml')
    res = record.record(watch, out / f'{a.name}.csv', frames=a.frames, state=a.state, pad=a.pad, pine=p)
    print(f'{res.frames} rows, frames {res.first}..{res.last}, {res.skipped} torn, {res.dropped} dropped; '
          f'ended: {res.ended}')
    if p.saved_at is None:
        print('no wreck: no state saved')
        return 1
    print(f'wreck reason {p.reason} first read at frame {p.wreck_at}')
    time.sleep(2.0)                                   # let the last save finish writing
    for v, slot in p.saved:
        dst = out / f'{a.name}-{v}.p2s'
        shutil.copyfile(emu.state_path(str(slot)), dst)
        print(f'  slot {slot} (asked at frame {v}) -> {dst.relative_to(ROOT)}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
