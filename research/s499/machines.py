"""S499: research/r22/machines.py over PINE on the running copy: the world's machine list (kind, actor,
driver, body, place, velocity, turn) and the cheat flag. With SLOT, also save the state there.

    ./kit pcsx2 with -- python3 -B research/s499/machines.py [SLOT]
"""
import struct
import sys
import time
sys.path.insert(0, '.')
from ps2kit.pine import Pine

p = Pine(check=False, lock=False, timeout=5.0)
f32s = lambda a, n: tuple(round(x, 4) for x in struct.unpack('<%df' % n, p.read_block(a, 4 * n)))
world = p.read32(p.read32(0x0041f770) + 0x3cc)
n = p.read32(world + 0x90)
print('frame %d world %#x, %d machines, cheat flag %d' % (p.frame(), world, n, p.read32(p.read32(0x004e2778) + 0x5c)))
for i in range(n):
    e = p.read32(p.read32(world + 0x98) + 4 * i)
    actor = p.read32(e + 8)
    body = p.read32(actor + 0x160) if actor else 0
    print(i, 'entry %#x kind %d actor %#x driver %#x body %#x awake %d' % (
        e, p.read32(e + 4), actor, p.read32(actor + 0x20) if actor else 0, body, p.read32(body) if body else 0))
    if body:
        print('   at', f32s(body + 0x60, 3), 'v', f32s(body + 0x70, 3), 'q', f32s(body + 0x170, 4))
if len(sys.argv) > 1:
    p.save_state(int(sys.argv[1]))
    time.sleep(2)
    print('saved', sys.argv[1])
