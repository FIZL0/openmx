"""S100: copy scratch slot SLOT (9: the teleported tree state) to context/s100/NAME.p2m2_SaveState.p2s and write
NAME.p2m2 beside it: full gas (Cross) for FIELDS fields (two a tick), from that state.
Usage: prep.py FIELDS [SLOT NAME]"""
import os, shutil, subprocess, sys
SAVES = '/home/rycs/repos/openmx-slots/s2-slots/pcsx2/s2/PCSX2/savestates/'
os.makedirs('context/s100', exist_ok=True)
slot, name = (int(sys.argv[2]), sys.argv[3]) if len(sys.argv) > 3 else (9, 'tree')
shutil.copyfile(SAVES + 'SLUS-20767 (9866FE54).%02d.p2s' % slot, 'context/s100/%s.p2m2_SaveState.p2s' % name)
subprocess.run([sys.executable, '-B', 'research/journal/2026-10-20-p21-p-a-top-speed-of-its-own/capture/make_p2m2.py',
                'context/recordings/recording2.p2m2', 'context/s100/%s.p2m2' % name, sys.argv[1]], check=True)
