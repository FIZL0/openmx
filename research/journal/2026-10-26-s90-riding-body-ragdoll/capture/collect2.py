"""Move the landing burst's PCSX2 snaps (taken today after argv[1] HHMMSS) to context/shots/s90/landing/pcsx2-<tick>.png.

    python3 collect2.py HHMMSS 138,142,145,...   (KIT_PCSX2 picks the copy, default 2)
"""
import os
import shutil
import sys
import time

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..', '..', '..'))
COPY = os.environ.get('PCSX2_CONFIG', os.path.expanduser(f"~/repos/openmx-slots/pcsx2/s{os.environ.get('KIT_PCSX2', '2')}/PCSX2"))
SNAPS = os.path.join(COPY, 'snaps')
OUT = os.path.join(ROOT, 'context', 'shots', 's90', 'landing')
TICKS = [int(t) for t in sys.argv[2].split(',')]
os.makedirs(OUT, exist_ok=True)
today = 'MX Unleashed_SLUS-20767_' + time.strftime('%Y%m%d')
files = sorted(f for f in os.listdir(SNAPS) if f.startswith(today) and f[-10:-4] >= sys.argv[1])
assert len(files) == len(TICKS), files
for t, f in zip(TICKS, files):
    shutil.move(os.path.join(SNAPS, f), os.path.join(OUT, f'pcsx2-{t}.png'))
    print(t, f)
