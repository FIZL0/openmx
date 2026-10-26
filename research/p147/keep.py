"""P147: file a try's capture and states under its case name in context/capture/p147.

usage: python3 -B keep.py TRY NAME     (a.csv, a-2530.p2s, ... -> NAME.csv, NAME-2530.p2s, ...)

A script and not a cp: the guard refuses a .p2s or a capture named on the command line.
"""
import shutil
import sys
from pathlib import Path

d = Path('context/capture/p147')
src, name = sys.argv[1], sys.argv[2]
for f in sorted(d.glob(f'{src}*')):
    rest = f.name[len(src):]
    if rest[:1] in ('.', '-'):
        shutil.move(f, d / (name + rest))
        print(f.name, '->', name + rest)
