"""Copy the user's truck tape (+ its state) from the main checkout into this slot's context (C12).

    python3 -B research/c12/fetch_truck.py
"""
import os
import shutil

SRC = os.path.expanduser('~/repos/openmx/context/recordings/split')
for n in ('truck.p2m2', 'truck.p2m2' + '_SaveState.p2s'):
    dst = os.path.join('context', 'recordings', 'split', n)
    shutil.copy(os.path.join(SRC, n), dst)
    os.chmod(dst, 0o644)
print(sorted(os.listdir(os.path.join('context', 'recordings', 'split'))))
