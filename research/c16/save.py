"""C16: save the running kit copy to SLOT. usage: save.py SLOT"""
import sys, time
sys.path.insert(0, '.')
from ps2kit.pine import Pine

p = Pine(check=False, lock=False, timeout=5.0)
print('frame', p.read32(0x00479d04), flush=True)
p.save_state(int(sys.argv[1]))
time.sleep(3)
print('saved slot', sys.argv[1], flush=True)
