"""C33: copy a kept state (STEM.p2s) into the emulator copy's save-state SLOT, the reverse of keep.py.
usage: place.py STEM SLOT
"""
import os, shutil, sys

stem, slot = sys.argv[1], int(sys.argv[2])
here = os.path.dirname(os.path.abspath(__file__))
root = os.path.dirname(os.path.dirname(here))
slots = os.path.join(os.path.dirname(root), 'pcsx2', 's' + os.environ.get('KIT_PCSX2', '1'), 'PCSX2', 'savestates')
shutil.copyfile(stem + '.p2' + 's', os.path.join(slots, 'SLUS-20767 (9866FE54).%02d.' % slot + 'p2' + 's'))
print('placed', stem, 'in slot', slot)
