"""Move the landing burst's PCSX2 snaps (after argv[1] HHMMSS) to context/shots/s90/landing/pcsx2-<tick>.png."""
import os
import shutil
import sys

SNAPS = '/home/rycs/repos/openmx-slots/pcsx2/s2/PCSX2/snaps'
OUT = '/home/rycs/repos/openmx-slots/s2/context/shots/s90/landing'
TICKS = [int(t) for t in sys.argv[2].split(',')]
os.makedirs(OUT, exist_ok=True)
files = sorted(f for f in os.listdir(SNAPS) if f.startswith('MX Unleashed_SLUS-20767_20261121') and f[-10:-4] >= sys.argv[1])
assert len(files) == len(TICKS), files
for t, f in zip(TICKS, files):
    shutil.move(os.path.join(SNAPS, f), os.path.join(OUT, f'pcsx2-{t}.png'))
    print(t, f)
