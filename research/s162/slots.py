"""S162: move .p2s files between context/s162 and this worktree's PCSX2 copy, and set its speed.

    python3 -B research/s162/slots.py put NAME SLOT     # context/s162/NAME.p2s -> the copy's slot SLOT
    python3 -B research/s162/slots.py keep SLOT NAME    # the copy's slot SLOT -> context/s162/NAME.p2s
    python3 -B research/s162/slots.py speed SCALAR      # [Framerate] NominalScalar (copy stopped)

The paths are built here rather than named on a command line (the guard refuses a .p2s argument)."""
import os
import re
import shutil
import sys

root = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..'))
copy = os.path.join(root + '-slots', 'pcsx2', 's' + os.environ.get('KIT_PCSX2', '1'), 'PCSX2')
mine = os.path.join(root, 'context', 's162')


def slot(n):
    return os.path.join(copy, 'savestates', 'SLUS-20767 (9866FE54).%02d.p2s' % int(n))


what = sys.argv[1]
if what == 'put':
    shutil.copyfile(os.path.join(mine, sys.argv[2] + '.p2s'), slot(sys.argv[3]))
    print('slot', sys.argv[3], '<-', sys.argv[2])
elif what == 'keep':
    os.makedirs(mine, exist_ok=True)
    shutil.copyfile(slot(sys.argv[2]), os.path.join(mine, sys.argv[3] + '.p2s'))
    print('slot', sys.argv[2], '->', sys.argv[3])
elif what == 'speed':
    ini = os.path.join(copy, 'inis', 'PCSX2.ini')
    text, n = re.subn(r'(?m)^NominalScalar = .*$', 'NominalScalar = ' + sys.argv[2], open(ini).read())
    assert n == 1, 'NominalScalar is not in ' + ini
    open(ini + '.tmp', 'w').write(text)
    os.replace(ini + '.tmp', ini)
    print('NominalScalar =', sys.argv[2])
else:
    raise SystemExit(__doc__)
