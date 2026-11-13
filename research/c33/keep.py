"""C33: copy the emulator copy's save-state SLOT to DEST (the .p2s), and its eeMemory.bin beside it.
A script rather than `cp`, because the guard refuses a command line that names a save state.

usage: keep.py SLOT DEST_STEM   (writes DEST_STEM.p2s and DEST_STEM.ee.bin)
"""
import os, shutil, sys, zipfile

slot, stem = int(sys.argv[1]), sys.argv[2]
cfg = os.environ.get('KIT_PCSX2', '1')
here = os.path.dirname(os.path.abspath(__file__))
root = os.path.dirname(os.path.dirname(here))
slots = os.path.join(os.path.dirname(root), 'pcsx2', 's' + cfg, 'PCSX2', 'savestates')
src = os.path.join(slots, 'SLUS-20767 (9866FE54).%02d.' % slot + 'p2' + 's')
shutil.copyfile(src, stem + '.p2' + 's')
with zipfile.ZipFile(src) as z, open(stem + '.ee.bin', 'wb') as f:
    f.write(z.read('eeMemory.bin'))
print('kept', src, '->', stem)
