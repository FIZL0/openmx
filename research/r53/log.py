"""R53: log P2's ride and the parked truck live, a line a game frame, for SECONDS wall seconds.

usage: python3 -B research/r53/log.py SECONDS     (under KIT_PCSX2=N ./kit pcsx2 with --)

P2's rider / vehicle as research/r53/tele_p2.py finds them; the wreck body is *(rider + 0x7a8) + 0x110
(+0x60 position, +0x70 velocity, research/s77_edge_back.py). PINE does not answer while the VM is paused
(the tape's load), so it waits through that.
"""
import struct
import sys
import time
sys.path.insert(0, '.')
from ps2kit.pine import Pine

FRAME, MANAGER, CAR_VTABLE, ACTOR_VTABLE = 0x00479d04, 0x004e1e98, 0x0045edac, 0x00458eb0
SECS = float(sys.argv[1])
ok = lambda a: 0x100000 <= a < 0x2000000
p = None
end = time.time() + SECS


def f3(a):
    return struct.unpack('<3f', p.read_block(a, 12))


def find():
    node = p.read32(p.read32(p.read32(MANAGER) + 0x34) + 0x34)
    rider = veh = truck = plane = None
    for _ in range(60):
        if not ok(node):
            break
        it = p.read32(node)
        if ok(it):
            pl = p.read32(it + 0x20)
            if p.read32(it) == 1 and plane is None:
                plane = it
            elif p.read32(it + 0x88) == CAR_VTABLE:
                truck = it
            elif ok(pl) and p.read32(pl + 0x30) == 1:
                if p.read32(it - 0x400 + 8) == ACTOR_VTABLE:
                    veh = it
                elif ok(p.read32(it + 0x7a4)):
                    rider = it
        node = p.read32(node + 0x8)
    return rider, veh, truck, plane


last = None
found = None
while time.time() < end:
    try:
        if p is None:
            p = Pine(check=False, lock=False, timeout=2.0)
        f = p.read32(FRAME)
        if f == last:
            time.sleep(0.005)
            continue
        if found is None or f < (last or 0):
            found = find()
        last = f
        rider, veh, truck, plane = found
        ch = p.read32(rider + 0x7a4)
        tb = p.read32(truck + 0x160)
        pb = p.read32(plane + 0x160)
        wb = p.read32(p.read32(rider + 0x7a8) + 0x110)
        w = f3(wb + 0x60) if ok(wb) else (0, 0, 0)
        wv = f3(wb + 0x70) if ok(wb) else (0, 0, 0)
        print('%6d reason %2d wrecked %d bike %8.2f %7.2f %8.2f sp %6.2f wreck %8.2f %7.2f %8.2f v %6.2f %6.2f %6.2f truck %d %8.2f %7.2f %8.2f v %6.2f %6.2f %6.2f plane %8.2f %7.2f %8.2f v %6.2f %6.2f %6.2f' % (
            (f, p.read32(rider + 0x748), p.read32(rider + 0xb7c)) + f3(veh + 0x1b0)
            + struct.unpack('<f', p.read_block(ch + 0x34, 4)) + w + wv + (p.read32(tb),) + f3(tb + 0x60) + f3(tb + 0x70)
            + f3(pb + 0x60) + f3(pb + 0x70)), flush=True)
    except (OSError, RuntimeError, ConnectionError):
        p = None
        time.sleep(0.2)
