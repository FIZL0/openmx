"""C16: wait until frame_counter reaches FRAME (or DEADLINE seconds pass). usage: wait_frame.py FRAME [DEADLINE]"""
import sys, time
sys.path.insert(0, '.')
from ps2kit.pine import Pine

want, deadline = int(sys.argv[1]), time.time() + float(sys.argv[2] if len(sys.argv) > 2 else 300)
p = Pine(check=False, lock=False, timeout=5.0)
f = 0
while time.time() < deadline:
    f = p.read32(0x00479d04)
    if f >= want:
        print('frame', f, flush=True)
        sys.exit(0)
    time.sleep(0.5)
sys.exit('timed out at frame %d' % f)
