"""Copy the human's biplane save state over copy 2's slot 8 (a scratch slot), for `./kit pine load 8`."""
import os
import shutil

src = os.path.join('context', 'recordings', 'biplane.p2m2' + '_SaveState.p2s')
dst = '/home/rycs/repos/openmx-slots/s2-slots/pcsx2/s2/PCSX2/savestates/SLUS-20767 (9866FE54).08.p2s'
shutil.copyfile(src, dst)
print('state in slot 8')
