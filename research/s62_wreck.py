"""S62: capture the original wrecking into the bike and through to the rider at rest.

usage: python3 -B research/s62_wreck.py NAME --pad SCRIPT [--state SLOT] [--frames N] [--late N] [--hold N]

`research/s56_wreck.py`'s capture (watch/s56-ragdoll.toml, one row a tick, save states from the tick
the wreck starts, here into slots 6, 7 and 8), written to context/capture/s62/NAME.csv, plus one more
state into slot 9 `--late` ticks after the wreck starts: what the wreck changed in RAM between the two
is what `tests/ragdoll_bike.rs` looks for the moving world in. `--hold N` saves the three states from
N ticks after the wreck instead, to compare the RAM at a tick where a replay parts. Every state is copied to
context/capture/s62/NAME-<frame asked>.p2s at the end.

Run it under `./kit pcsx2 with --`, with `./kit pad serve` up for the copy, from recording2's save
state in `--state` (default 3).
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
from s56_wreck import Saving             # noqa: E402

ROOT = Path(__file__).resolve().parent.parent


class Late(Saving):
    """S56's saver, and one state more `late` ticks after the wreck into `slot`."""

    def __init__(self, slots: list[int], late: int, slot: int, hold: int = 0):
        super().__init__(slots if hold == 0 else [])
        self.late, self.late_slot = late, slot
        self.held = list(slots) if hold else None
        self.hold = hold

    def next_frame(self, last, *a, **k):
        v = super().next_frame(last, *a, **k)
        if self.held and self.wreck_at is not None and v + 1 >= self.wreck_at + self.hold:
            self.slots, self.held = self.held, None       # the next ticks save them
        if self.wreck_at is not None and self.late_slot is not None and v >= self.wreck_at + self.late:
            self.save_state(self.late_slot)
            self.saved.append((v, self.late_slot))
            print(f'  frame {v}: saving the late state into slot {self.late_slot}', flush=True)
            self.late_slot = None
        return v


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument('name')
    ap.add_argument('--pad', required=True)
    ap.add_argument('--state', type=int, default=3)
    ap.add_argument('--frames', type=int, default=800)
    ap.add_argument('--late', type=int, default=400, help='ticks after the wreck for the last state')
    ap.add_argument('--hold', type=int, default=0, help='ticks after the wreck for the first three states')
    a = ap.parse_args()
    out = ROOT / 'context' / 'capture' / 's62'
    out.mkdir(parents=True, exist_ok=True)
    p = Late([6, 7, 8], a.late, 9, a.hold)
    watch = record.Watch(ROOT / 'watch' / 's56-ragdoll.toml')
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
