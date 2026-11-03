"""S241: move .p2s files between context/ and this worktree's PCSX2 copy.

    python3 -B research/s252/slots.py put biplane SLOT     # context/recordings/biplane.p2m2_SaveState.p2s
    python3 -B research/s252/slots.py put NAME SLOT        # context/s252/NAME.p2s -> the copy's slot SLOT
    python3 -B research/s252/slots.py keep SLOT NAME       # the copy's slot SLOT -> context/s252/NAME.p2s
    python3 -B research/s252/slots.py ram NAME             # context/s252/NAME.p2s -> context/states/s252-NAME.eeMemory.bin

research/s241/slots.py's, for context/s252. The paths are built here rather than named on a command line
(the guard refuses a .p2s argument). KIT_PCSX2 names the copy (default 1)."""
import os
import shutil
import sys
import zipfile

root = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..'))
copy = os.path.join(root + '-slots', 'pcsx2', 's' + os.environ.get('KIT_PCSX2', '1'), 'PCSX2')
mine = os.path.join(root, 'context', 's252')


def slot(n):
    return os.path.join(copy, 'savestates', 'SLUS-20767 (9866FE54).%02d.p2s' % int(n))


what = sys.argv[1]
if what == 'put':
    name = sys.argv[2]
    src = (os.path.join(root, 'context', 'recordings', name + '.p2m2_SaveState.p2s') if name == 'biplane'
           else os.path.join(mine, name + '.p2s'))
    shutil.copyfile(src, slot(sys.argv[3]))
    print('slot', sys.argv[3], '<-', name)
elif what == 'keep':
    os.makedirs(mine, exist_ok=True)
    shutil.copyfile(slot(sys.argv[2]), os.path.join(mine, sys.argv[3] + '.p2s'))
    print('slot', sys.argv[2], '->', sys.argv[3])
elif what == 'ram':
    out = os.path.join(root, 'context', 'states', 's252-' + sys.argv[2] + '.eeMemory.bin')
    with zipfile.ZipFile(os.path.join(mine, sys.argv[2] + '.p2s')) as z, open(out, 'wb') as f:
        f.write(z.read('eeMemory.bin'))
    print(sys.argv[2], '->', os.path.relpath(out, root))
else:
    raise SystemExit(__doc__)
