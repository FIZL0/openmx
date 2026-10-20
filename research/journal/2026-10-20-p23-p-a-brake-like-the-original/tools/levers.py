"""Print bike+0x63c..+0x65c (gas, brake, front brake, rear brake, throttle, clutch) each tick in a window.

usage: levers.py BIKE FIRST LAST
"""
import struct, sys, time
sys.path.insert(0, '/home/rycs/repos/openmx-slots/s1')
from ps2kit.pine import Pine
bike, lo, hi = int(sys.argv[1], 16), int(sys.argv[2]), int(sys.argv[3])
p = Pine()
FC = 0x479d04
last = None
t0 = time.time()
while time.time() - t0 < 240:
    f = p.read32(FC)
    if f == last:
        time.sleep(0.002)
        continue
    last = f
    if f < lo or f > hi:
        if f > hi and f < hi + 1000:
            break
        continue
    blk = p.read_block(bike + 0x630, 0x30)
    v = struct.unpack('<12f', blk)
    print(f, ' '.join(f'{x:8.3f}' for x in v), flush=True)
