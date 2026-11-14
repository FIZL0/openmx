"""MD160: copy copy 3's Untamed states (slots 12-17), the md160 captures and tapes into
kept-context/md160 and the states into context/untamed/md160.

  python3 -I research/md160/keep.py
"""
import os
import shutil

HOME = os.path.expanduser('~')
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
SAVES = os.path.join(HOME, 'repos/openmx-slots/pcsx2/s3/PCSX2/savestates')
KEEP = os.path.join(HOME, 'repos/openmx-slots/kept-context/md160')
NAMES = {12: 'options', 13: 'vehicle-atv', 14: 'vehicle-truck', 15: 'spawn', 16: 'air', 17: 'flipped'}
for d in (KEEP, os.path.join(ROOT, 'context/untamed/md160')):
    os.makedirs(d, exist_ok=True)
    for slot, name in NAMES.items():
        shutil.copy2(os.path.join(SAVES, 'SLUS-21701 (08FE7DFD).%02d.p2s' % slot), os.path.join(d, name + '.p2s'))
for sub in ('capture', 'recordings'):
    src = os.path.join(ROOT, 'context', sub, 'md160')
    dst = os.path.join(KEEP, sub)
    os.makedirs(dst, exist_ok=True)
    for f in sorted(os.listdir(src)):
        shutil.copy2(os.path.join(src, f), os.path.join(dst, f))
for root, _, files in os.walk(KEEP):
    for f in sorted(files):
        print(os.path.relpath(os.path.join(root, f), KEEP), os.path.getsize(os.path.join(root, f)))
