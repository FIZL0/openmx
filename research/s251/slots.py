"""S251: move .p2s files between context/ and this worktree's PCSX2 copy (research/s252/slots.py's, for
context/s251, the copy's folder from ps2kit).

    python3 -B research/s251/slots.py along SLOT        # kept-context/r32/s89-along.p2s -> the copy's slot SLOT
    python3 -B research/s251/slots.py keep SLOT NAME    # the copy's slot SLOT -> context/s251/NAME.p2s
    python3 -B research/s251/slots.py put NAME SLOT     # context/s251/NAME.p2s -> the copy's slot SLOT

The paths are built here rather than named on a command line (the guard refuses a .p2s argument).
KIT_PCSX2 names the copy. RAM images: `./kit state ram SLOT --out context/states/s251-NAME.eeMemory.bin`."""
import os
import shutil
import sys

root = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..'))
sys.path.insert(0, root)
from ps2kit import platform_ as plat     # noqa: E402

copy = plat.pcsx2_config_dir(os.environ['KIT_PCSX2'])
mine = os.path.join(root, 'context', 's251')
kept = os.path.join(os.path.dirname(os.path.dirname(root)), 'openmx-slots', 'kept-context', 'r32')


def slot(n):
    return os.path.join(copy, 'savestates', 'SLUS-20767 (9866FE54).%02d.p2s' % int(n))


what = sys.argv[1]
if what == 'along':
    shutil.copyfile(os.path.join(kept, 's89-along.p2s'), slot(sys.argv[2]))
    print('slot', sys.argv[2], '<- kept-context/r32/s89-along')
elif what == 'keep':
    os.makedirs(mine, exist_ok=True)
    shutil.copyfile(slot(sys.argv[2]), os.path.join(mine, sys.argv[3] + '.p2s'))
    print('slot', sys.argv[2], '->', sys.argv[3])
elif what == 'put':
    shutil.copyfile(os.path.join(mine, sys.argv[2] + '.p2s'), slot(sys.argv[3]))
    print('slot', sys.argv[3], '<-', sys.argv[2])
else:
    raise SystemExit(__doc__)
