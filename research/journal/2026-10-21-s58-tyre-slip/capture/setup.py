"""S58: ready this worktree's PCSX2 copy for a slip capture (run with the copy stopped).

    python3 -B setup.py SCALAR      # 0.25 to capture, 1.0 to put it back

Copies first_recording's save state over slot 8 and sets [Framerate] NominalScalar. Both paths are
built here rather than named on a command line (the guard refuses a .p2s argument)."""
import os
import re
import shutil
import sys

here = os.path.dirname(os.path.abspath(__file__))
root = os.path.abspath(os.path.join(here, '..', '..', '..', '..'))
copy = os.path.join(root + '-slots', 'pcsx2', 's' + os.environ.get('KIT_PCSX2', '1'), 'PCSX2')
scalar = sys.argv[1]

state = os.path.join(root, 'context', 'recordings', 'first_recording.p2m2_SaveState.p2s')
slot = os.path.join(copy, 'savestates', 'SLUS-20767 (9866FE54).08.p2s')
shutil.copyfile(state, slot)

ini = os.path.join(copy, 'inis', 'PCSX2.ini')
text = open(ini).read()
text, n = re.subn(r'(?m)^NominalScalar = .*$', 'NominalScalar = ' + scalar, text)
assert n == 1, 'NominalScalar is not in ' + ini
open(ini + '.tmp', 'w').write(text)
os.replace(ini + '.tmp', ini)
print('slot 8 <- first_recording; NominalScalar =', scalar)
