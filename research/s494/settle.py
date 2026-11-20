"""S494: once research/r52/boot.py is in game, ride FRAMES more frames and save state SLOT (boot.py's
own wait gives up when the frame counter rests at 0 through the slow copy's load).

usage: settle.py SLOT FRAMES
"""
import sys
import time
sys.path.insert(0, '.')
from ps2kit.pine import Pine

slot, frames = int(sys.argv[1]), int(sys.argv[2])
p = Pine()
t0 = time.time()
f = p.read32(0x00479d04)
while p.read32(0x00479d04) - f < frames:
    if time.time() - t0 > 900:
        raise SystemExit('frames stalled')
    time.sleep(0.25)
print('frame', p.read32(0x00479d04), flush=True)
p.save_state(slot)
time.sleep(3)
print('saved', slot, flush=True)
