"""Turn research/c10/at_ticks.py's saves from this slot's copy into RAM images (S158):
context/states/s158/<ride>-<frame>.ram.

    python3 -B research/s158/rams.py RIDE LOG     # LOG: the at_ticks.py output (`tick slot at counter`)

C10's rams.py (research/c10/rams.py) with kit copy 1's saves and the ride's name in front. Each image
is named by the frame_counter it holds, which may not be the tick asked for (said so). Research, not
product.
"""
import glob
import os
import re
import struct
import subprocess
import sys

FRAME_COUNTER = 0x00479d04
SAVES = os.path.join('..', 'pcsx2', 's1', 'PCSX2', 'savestates')
ride, log = sys.argv[1:3]
out_dir = os.path.join('context', 'states', 's158')
os.makedirs(out_dir, exist_ok=True)
for line in open(log):
    m = re.match(r'(\d+) (\d+) at (\d+)$', line.strip())
    if not m:
        continue
    tick, slot = int(m.group(1)), int(m.group(2))
    tmp = os.path.join(out_dir, 'slot-%d.ram' % slot)
    p2s = glob.glob(os.path.join(SAVES, '*.%02d.p2s' % slot))[0]
    subprocess.run(['./kit', 'state', 'ram', p2s, '--out', tmp], check=True, stdout=subprocess.DEVNULL)
    with open(tmp, 'rb') as f:
        f.seek(FRAME_COUNTER)
        held = struct.unpack('<I', f.read(4))[0]
    name = os.path.join(out_dir, '%s-%d.ram' % (ride, held))
    os.replace(tmp, name)
    print('%d %s %d%s' % (tick, name, held, '' if held == tick else '  (not the tick asked for)'), flush=True)
