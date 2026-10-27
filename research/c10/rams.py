"""Turn research/c10/at_ticks.py's saves into RAM images (C10): context/states/c10/c10-<tick>.ram.

    python3 -B research/c10/rams.py LOG       # the at_ticks.py output (`tick slot at counter` lines)

Unpacks each slot with `./kit state ram`, reads the image's own frame_counter, and names the image by the
tick asked for when it holds that tick; otherwise by the tick it holds, and says so. Prints `tick image
counter` per save. Research, not product.
"""
import os
import glob
import re
import struct
import subprocess
import sys

FRAME_COUNTER = 0x00479d04
# kit copy 2's saves (KIT_PCSX2=2)
SAVES = os.path.join('..', 's2-slots', 'pcsx2', 's2', 'PCSX2', 'savestates')
out_dir = os.path.join('context', 'states', 'c10')
os.makedirs(out_dir, exist_ok=True)
for line in open(sys.argv[1]):
    m = re.match(r'(\d+) (\d+) at (\d+)$', line.strip())
    if not m:
        continue
    tick, slot = int(m.group(1)), int(m.group(2))
    tmp = os.path.join(out_dir, 'slot-%d.ram' % slot)
    # `./kit state ram N` finds two-digit slots only: name the file
    p2s = glob.glob(os.path.join(SAVES, '*.%02d.p2s' % slot))[0]
    subprocess.run(['./kit', 'state', 'ram', p2s, '--out', tmp], check=True, stdout=subprocess.DEVNULL)
    with open(tmp, 'rb') as f:
        f.seek(FRAME_COUNTER)
        held = struct.unpack('<I', f.read(4))[0]
    name = os.path.join(out_dir, 'c10-%d.ram' % held)
    os.replace(tmp, name)
    print('%d %s %d%s' % (tick, name, held, '' if held == tick else '  (not the tick asked for)'), flush=True)
