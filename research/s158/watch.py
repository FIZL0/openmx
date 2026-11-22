"""S158: print both riders' chassis centres, speeds and wreck reasons while a 2p tape plays (scouting).

    python3 -B research/s158/watch.py SECONDS [EVERY]

Polls over PINE (the only client): the players by two_p_world.rs's chain off the game manager, each
line `frame  p1 x y z |v| wreck  p2 x y z |v| wreck  distance`. Research, not product.
"""
import math
import os
import struct
import sys
import time

sys.path.insert(0, os.getcwd())
from ps2kit.pine import Pine  # noqa: E402

GAME_MANAGER = 0x004e1e98
FRAME_COUNTER = 0x00479d04
secs = float(sys.argv[1])
every = float(sys.argv[2]) if len(sys.argv) > 2 else 0.25
p = Pine()


def down(hops):
    a = p.read32(GAME_MANAGER)
    for h in hops:
        a = p.read32(a + h)
    return a


end = time.time() + secs
while time.time() < end:
    try:
        f = p.read32(FRAME_COUNTER)
        out, cs = [], []
        for hops in ([0x34, 0x34, 0x8, 0x8, 0x8, 0x0], [0x34, 0x34, 0x8, 0x8, 0x0]):
            v = down(hops)
            ch = p.read32(v + 0x7a4)
            c = struct.unpack('<3f', p.read_block(ch + 0x60, 12))
            vel = struct.unpack('<3f', p.read_block(ch + 0x260, 12))
            cs.append(c)
            out.append('%9.2f %7.2f %9.2f %6.1f w%-2d' % (*c, math.hypot(*vel), p.read32(v + 0x748)))
        print(f, '  '.join(out), '%7.1f' % math.dist(*cs), flush=True)
    except Exception as e:  # noqa: BLE001 - a load or a torn chain mid-frame
        print('read failed:', e, flush=True)
    time.sleep(every)
