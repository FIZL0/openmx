"""C43: record SPEC into OUT for N rows, saving PCSX2 slots 3.. at the frames asked, then keep each as
context/c43/NAME-<frame asked>.p2s and its RAM image as context/states/c43-NAME-<frame asked>.ee.bin/.spr.bin.

usage: python3 -B research/c43/strike.py SPEC OUT N NAME FRAME[,FRAME...]

research/s195/knock.py's way: the save is made from the recorder's PINE client in its frame hook. A save
asked at frame F holds the RAM of a later row (F27a1's images were row F+1): match each image to the csv
by the helicopter's body (research/c43/images.py) before naming what it shows. fly.sh calls this when
C43_AT is set. Run under `KIT_PCSX2=N ./kit pcsx2 with --`.
"""

import shutil
import sys
import time
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(ROOT))
from ps2kit import emu                   # noqa: E402
from ps2kit import pine as pine_mod      # noqa: E402
from ps2kit import record                # noqa: E402

SPEC, OUT, N, NAME = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4]
AT = [int(v) for v in sys.argv[5].split(',')]


class Saver(pine_mod.Pine):
    def __init__(self):
        super().__init__(check=True)
        self.todo = dict(zip(AT, range(3, 3 + len(AT))))
        self.saved: list[tuple[int, int]] = []

    def next_frame(self, last, *a, **k):
        v = super().next_frame(last, *a, **k)
        slot = self.todo.pop(v, None)
        if slot is not None:
            self.save_state(slot)
            self.saved.append((v, slot))
            print(f'  frame {v}: saving into slot {slot}', flush=True)
        return v


p = Saver()
res = record.record(record.Watch(SPEC), OUT, frames=N, pine=p)
print(f'{res.frames} rows, frames {res.first}..{res.last}, {res.skipped} torn, {res.dropped} dropped; '
      f'ended: {res.ended}')
time.sleep(3.0)                                       # let the last save finish writing
keep = ROOT / 'context' / 'c43'
keep.mkdir(parents=True, exist_ok=True)
states = ROOT / 'context' / 'states'
for v, slot in p.saved:
    dst = keep / f'{NAME}-{v}.p2s'
    shutil.copyfile(emu.state_path(str(slot)), dst)
    with zipfile.ZipFile(dst) as z:
        (states / f'c43-{NAME}-{v}.ee.bin').write_bytes(z.read('eeMemory.bin'))
        (states / f'c43-{NAME}-{v}.spr.bin').write_bytes(z.read('Scratchpad.bin'))
    print(f'  slot {slot} (asked at frame {v}) -> {dst.relative_to(ROOT)}, context/states/c43-{NAME}-{v}.ee.bin')
if len(p.saved) != len(AT):
    print('missed frames:', sorted(p.todo))
    sys.exit(1)
