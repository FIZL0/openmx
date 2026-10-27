"""Extract a tape's save-state EE RAM to context/states/c12/<name>-start.ram (C12), for research/c10/records.py.

    python3 -B research/c12/state_ram.py truck
"""
import os
import subprocess
import sys

src = os.path.join('context', 'recordings', 'split', sys.argv[1] + '.p2m2' + '_SaveState.p2s')
os.makedirs(os.path.join('context', 'states', 'c12'), exist_ok=True)
out = os.path.join('context', 'states', 'c12', sys.argv[1] + '-start.ram')
subprocess.run(['./kit', 'state', 'ram', src, '--out', out], check=True)
print(out)
