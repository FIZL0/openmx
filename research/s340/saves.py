"""S340: record a spec as `./kit record` does, saving states on chosen frame_counter values.

usage: python3 -B research/s340/saves.py SPEC.toml N OUT.csv FC[,FC...]

Each FC: the first tick the recorder sees frame_counter (0x479d04) >= FC, save into slots 3, 4, 5, ...;
copied to OUT's folder as <OUT stem>-<frame_counter seen>.p2s at the end. Run with the copy's KIT_CONFIG,
from research/s340/capture.sh.
"""

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

FRAME_COUNTER = 0x479d04


class Saves(pine_mod.Pine):
    def __init__(self, at: list[int]):
        super().__init__(check=True)
        self.at = sorted(at)
        self.slots = list(range(3, 3 + len(at)))
        self.saved: list[tuple[int, int]] = []

    def next_frame(self, last, *a, **k):
        v = super().next_frame(last, *a, **k)
        fc = struct.unpack('<I', self.read_block(FRAME_COUNTER, 4))[0]
        if self.at and fc >= self.at[0]:
            self.at.pop(0)
            slot = self.slots.pop(0)
            self.save_state(slot)
            self.saved.append((fc, slot))
            print(f'  frame_counter {fc}: saved into slot {slot}', flush=True)
        return v


def main() -> int:
    spec, n, out, at = sys.argv[1], int(sys.argv[2]), Path(sys.argv[3]), [int(x) for x in sys.argv[4].split(',')]
    p = Saves(at)
    res = record.record(record.Watch(spec), out, frames=n, pine=p)
    print(f'{res.frames} rows, {res.skipped} torn, {res.dropped} dropped; ended: {res.ended}', flush=True)
    time.sleep(3.0)
    for fc, slot in p.saved:
        dst = out.parent / f'{out.stem}-{fc}.p2s'
        shutil.copyfile(emu.state_path(str(slot)), dst)
        print(f'  slot {slot} (frame_counter {fc}) -> {dst}', flush=True)
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
