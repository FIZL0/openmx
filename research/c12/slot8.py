"""Put a tape's save state over the kit copy's slot 8 (C12), for `./kit pine load 8`.

    python3 -B research/c12/slot8.py truck      # context/recordings/split/truck.p2m2's state
"""
import os
import shutil
import sys

src = os.path.join('context', 'recordings', 'split', sys.argv[1] + '.p2m2' + '_SaveState.p2s')
dst = os.path.expanduser('~/repos/openmx-slots/s1-slots/pcsx2/s1/PCSX2/savestates/SLUS-20767 (9866FE54).08.p2s')
shutil.copy(src, dst)
print(dst)
