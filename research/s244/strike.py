"""S244: record a free09 ball struck by the wreck bike from research/s197b/put_ball.py's state, saving a
state on every one of the first eight ticks.

usage: python3 -B research/s244/strike.py NAME SLOT [--frames N]

watch/s197b-wreck.toml, one row a tick, from save state SLOT into context/capture/s244/NAME.csv; slots 3
to 9 and 0 are the first eight ticks after the load, copied to context/s244/NAME-<frame asked>.p2s at the
end, and SLOT itself (the tick before them) to context/s244/NAME-put.p2s. research/s197b/wreck.py's
recorder, with no distance or wake test: the strike is on the first tick after put_ball.py's move.

Run it under `KIT_PCSX2=N ./kit pcsx2 with --`, at a quarter speed, with the game paused or running.
"""

import argparse
import shutil
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(ROOT))
from ps2kit import emu                   # noqa: E402
from ps2kit import pine as pine_mod      # noqa: E402
from ps2kit import record                # noqa: E402


class Every(pine_mod.Pine):
    def __init__(self):
        super().__init__(check=True)
        self.slots = [3, 4, 5, 6, 7, 8, 9, 0]
        self.saved: list[tuple[int, int]] = []

    def next_frame(self, last, *a, **k):
        v = super().next_frame(last, *a, **k)
        if self.slots:
            slot = self.slots.pop(0)
            self.save_state(slot)
            self.saved.append((v, slot))
            print(f'  frame {v}: saving into slot {slot}', flush=True)
        return v


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument('name')
    ap.add_argument('slot', type=int)
    ap.add_argument('--frames', type=int, default=60)
    a = ap.parse_args()
    out = ROOT / 'context' / 'capture' / 's244'
    out.mkdir(parents=True, exist_ok=True)
    keep = ROOT / 'context' / 's244'
    keep.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(emu.state_path(str(a.slot)), keep / f'{a.name}-put.p2s')
    p = Every()
    watch = record.Watch(ROOT / 'watch' / 's197b-wreck.toml')
    res = record.record(watch, out / f'{a.name}.csv', frames=a.frames, state=a.slot, pine=p)
    print(f'{res.frames} rows, frames {res.first}..{res.last}, {res.skipped} torn, {res.dropped} dropped; '
          f'ended: {res.ended}')
    time.sleep(2.0)                                   # let the last save finish writing
    for v, slot in p.saved:
        dst = keep / f'{a.name}-{v}.p2s'
        shutil.copyfile(emu.state_path(str(slot)), dst)
        print(f'  slot {slot} (asked at frame {v}) -> {dst.relative_to(ROOT)}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
