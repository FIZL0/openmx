"""S129: copy a .p2s into kit copy 2's save-state slot. usage: put_state.py SRC SLOT"""
import shutil, sys
from pathlib import Path
src, slot = sys.argv[1], int(sys.argv[2])
dst = Path('/home/rycs/repos/openmx-slots/s2-slots/pcsx2/s2/PCSX2/savestates') / ('SLUS-20767 (9866FE54).%02d.p2s' % slot)
shutil.copyfile(src, dst)
print(dst, dst.stat().st_size)
