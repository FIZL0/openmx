"""S240: move .p2s files between context/ and this worktree's PCSX2 copy, and set its speed.

    python3 -B research/s240/slots.py put first_recording SLOT  # context/recordings/first_recording.p2m2_SaveState.p2s
    python3 -B research/s240/slots.py put NAME SLOT             # context/s240/NAME.p2s (s197b-NAME: context/s197b's) -> slot SLOT
    python3 -B research/s240/slots.py keep SLOT NAME            # the copy's slot SLOT -> context/s240/NAME.p2s
    python3 -B research/s240/slots.py ram NAME                  # context/s240/NAME.p2s -> context/states/s240-NAME.eeMemory.bin
    python3 -B research/s240/slots.py speed SCALAR              # [Framerate] NominalScalar (copy stopped)

research/s197b/slots.py's, for context/s240. The paths are built here rather than named on a command line
(the guard refuses a .p2s argument). KIT_PCSX2 names the copy (default 1)."""
import os
import re
import shutil
import sys
import zipfile

root = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..'))
copy = os.path.join(root + '-slots', 'pcsx2', 's' + os.environ.get('KIT_PCSX2', '1'), 'PCSX2')
mine = os.path.join(root, 'context', 's240')


def slot(n):
    return os.path.join(copy, 'savestates', 'SLUS-20767 (9866FE54).%02d.p2s' % int(n))


what = sys.argv[1]
if what == 'put':
    name = sys.argv[2]
    src = (os.path.join(root, 'context', 'recordings', name + '.p2m2_SaveState.p2s') if name == 'first_recording'
           else os.path.join(root, 'context', 's197b', name[6:] + '.p2s') if name.startswith('s197b-')
           else os.path.join(mine, name + '.p2s'))
    shutil.copyfile(src, slot(sys.argv[3]))
    print('slot', sys.argv[3], '<-', name)
elif what == 'keep':
    os.makedirs(mine, exist_ok=True)
    shutil.copyfile(slot(sys.argv[2]), os.path.join(mine, sys.argv[3] + '.p2s'))
    print('slot', sys.argv[2], '->', sys.argv[3])
elif what == 'ram':
    out = os.path.join(root, 'context', 'states', 's240-' + sys.argv[2] + '.eeMemory.bin')
    with zipfile.ZipFile(os.path.join(mine, sys.argv[2] + '.p2s')) as z, open(out, 'wb') as f:
        f.write(z.read('eeMemory.bin'))
    print(sys.argv[2], '->', os.path.relpath(out, root))
elif what == 'speed':
    ini = os.path.join(copy, 'inis', 'PCSX2.ini')
    text, n = re.subn(r'(?m)^NominalScalar = .*$', 'NominalScalar = ' + sys.argv[2], open(ini).read())
    assert n == 1, 'NominalScalar is not in ' + ini
    open(ini + '.tmp', 'w').write(text)
    os.replace(ini + '.tmp', ini)
    print('NominalScalar =', sys.argv[2])
else:
    raise SystemExit(__doc__)
