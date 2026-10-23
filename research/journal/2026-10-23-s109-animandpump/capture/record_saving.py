"""S109: `./kit record`, saving a PCSX2 state on chosen ticks while it records.

usage: python3 -B record_saving.py SPEC OUT.csv FRAMES [--save FRAME:SLOT,...]

Like research/s56_wreck.py, but the states are asked for on the listed frame counters rather than on
a wreck. PCSX2 saves at its next safe point, which can be a tick later than the one asked for; the
test finds each state's own tick by its frame counter. Each slot is copied to OUT-<frame>.p2s at the
end. Run it from the repo root, inside `./kit pcsx2 with`.
"""

import argparse
import shutil
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT))
from ps2kit import emu                   # noqa: E402
from ps2kit import pine as pine_mod      # noqa: E402
from ps2kit import record                # noqa: E402


class Saving(pine_mod.Pine):
    def __init__(self, at: dict[int, int]):
        super().__init__(check=True)
        self.at = dict(at)
        self.saved: list[tuple[int, int]] = []

    def next_frame(self, last, *a, **k):
        v = super().next_frame(last, *a, **k)
        for frame in sorted(self.at):
            if v >= frame:
                slot = self.at.pop(frame)
                self.save_state(slot)
                self.saved.append((frame, slot))
                print(f'  frame {v}: saving a state into slot {slot} (asked for {frame})', flush=True)
                break
        return v


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument('spec')
    ap.add_argument('out')
    ap.add_argument('frames', type=int)
    ap.add_argument('--save', default='')
    a = ap.parse_args()
    at = {int(f): int(s) for f, s in (x.split(':') for x in a.save.split(',') if x)}
    p = Saving(at)
    out = Path(a.out)
    res = record.record(record.Watch(a.spec), out, frames=a.frames, pine=p)
    print(f'{res.frames} rows, frames {res.first}..{res.last}, {res.skipped} torn, {res.dropped} dropped; '
          f'ended: {res.ended}', flush=True)
    time.sleep(3.0)                                   # let the last save finish writing
    for frame, slot in p.saved:
        dst = out.with_name(f'{out.stem}-{frame}.p2s')
        shutil.copyfile(emu.state_path(str(slot)), dst)
        print(f'  slot {slot} (asked at frame {frame}) -> {dst}', flush=True)
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
