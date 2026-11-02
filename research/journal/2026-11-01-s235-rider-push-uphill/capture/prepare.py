"""S235: put TAPE and copy 1's save-state SLOT side by side in WORK as play.p2m2 and its state, both
writable (PCSX2 records over what it plays and never plays a read-only copy), and the spec with its
`start` trigger armed on FIRST. Done in Python: the guard refuses a cp naming a .p2s.

    python3 prepare.py TAPE SLOT FIRST WORK"""
import os, shutil, stat, sys
tape, slot, first, work = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), sys.argv[4]
here = os.path.dirname(os.path.abspath(__file__))
root = os.path.abspath(os.path.join(here, '..', '..', '..', '..'))
states = os.path.join(root + '-slots', 'pcsx2', 's' + os.environ.get('KIT_PCSX2', '1'), 'PCSX2', 'savestates')
shutil.copyfile(tape, os.path.join(work, 'play.p2m2'))
shutil.copyfile(os.path.join(states, 'SLUS-20767 (9866FE54).%02d.p2s' % slot), os.path.join(work, 'play.p2m2_SaveState.p2s'))
for f in ('play.p2m2', 'play.p2m2_SaveState.p2s'):
    os.chmod(os.path.join(work, f), stat.S_IRUSR | stat.S_IWUSR)
spec = open(os.path.join(here, 'climb.toml')).read()
trigger = 'start       = "frame_counter >= %d and frame_counter <= %d and bike_ctl_src == 1"' % (first, first + 26)
spec = spec.replace('every       = 1\n', 'every       = 1\n' + trigger + '\n', 1)
assert trigger in spec
open(os.path.join(work, 'spec.toml'), 'w').write(spec)
print('ready in', work)
