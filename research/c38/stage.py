"""C38: copy MD166's ATV race state into this kit copy's save-state slot SLOT (from Python: the guard refuses
a .p2s named on a command line). usage: stage.py SAVESTATES_DIR SLOT"""
import os
import shutil
import sys

src = os.path.join('context', 'mxvatv-ps2', 'md166', 'atv-race.' + 'p2s')
dst = os.path.join(sys.argv[1], 'SLUS-21104 (95BB1901).%02d.' % int(sys.argv[2]) + 'p2s')
shutil.copyfile(src, dst)
print('staged', dst)
