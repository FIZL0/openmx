"""S129: keep kit copy 2's save-state slot SLOT as context/s129/NAME.p2s. usage: keep_state.py SLOT NAME"""
import shutil, sys
from pathlib import Path
slot, name = int(sys.argv[1]), sys.argv[2]
src = Path('/home/rycs/repos/openmx-slots/s2-slots/pcsx2/s2/PCSX2/savestates') / ('SLUS-20767 (9866FE54).%02d.p2s' % slot)
shutil.copyfile(src, Path('context/s129') / (name + '.p2s'))
print('kept', name)
