"""S149a: copy kit copy 1's save slot SLOT to context/s149a/NAME.p2m2_SaveState.p2s and write the
generated tape NAME.p2m2 beside it (make_tape.py, header from first_recording.p2m2).

usage: prep.py SLOT NAME [STEER]   (STEER: make_tape.py's)"""
import os, shutil, subprocess, sys
here = os.path.dirname(os.path.abspath(__file__))
SAVES = os.path.abspath('.') + '-slots/pcsx2/s1/PCSX2/savestates/'
slot, name = int(sys.argv[1]), sys.argv[2]
os.makedirs('context/s149a', exist_ok=True)
shutil.copyfile(SAVES + 'SLUS-20767 (9866FE54).%02d.p2s' % slot, 'context/s149a/%s.p2m2_SaveState.p2s' % name)
subprocess.run([sys.executable, '-B', os.path.join(here, 'make_tape.py'), 'context/recordings/first_recording.p2m2',
                'context/s149a/%s.p2m2' % name, *sys.argv[3:]], check=True)
