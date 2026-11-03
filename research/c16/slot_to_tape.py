"""C16: copy a kit copy's save-state slot beside a tape as its _SaveState.p2s (the guard refuses a .p2s
named on a command line). usage: slot_to_tape.py SAVESTATES_DIR SLOT TAPE.p2m2"""
import os, shutil, sys

src = os.path.join(sys.argv[1], 'SLUS-20767 (9866FE54).%02d.p2s' % int(sys.argv[2]))
shutil.copyfile(src, sys.argv[3] + '_SaveState.p2s')
print(sys.argv[3] + '_SaveState.p2s', os.path.getsize(src), 'bytes')
