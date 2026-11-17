"""N24: log both planes (actor entries 0 and 1) live while a tape plays, one line every EVERY frames, until
frame_counter reaches END (or DEADLINE seconds). Research, not a capture: rows can tear.
research/c16/fly_log.py for two planes; each line names the plane's player (record +0x30) and control
source (+0x34; 1 = a local human, 2 = parked).

usage: log2.py START END [EVERY] [DEADLINE]   (waits for the load: a frame within START..START+30)
"""
import math, struct, sys, time
sys.path.insert(0, '.')
from ps2kit.pine import Pine

start, end = int(sys.argv[1]), int(sys.argv[2])
every = int(sys.argv[3]) if len(sys.argv) > 3 else 15
deadline = time.time() + float(sys.argv[4] if len(sys.argv) > 4 else 900)
p = Pine(check=False, lock=False, timeout=5.0)
f32 = lambda a: struct.unpack('<f', struct.pack('<I', p.read32(a)))[0]
v3 = lambda a: tuple(f32(a + 4 * i) for i in range(3))


def actor(i):
    node = p.read32(p.read32(p.read32(0x004e1e98) + 0x34) + 0x34)
    for _ in range(i):
        node = p.read32(node + 8)
    return p.read32(node)


def plane(a):
    body = p.read32(a + 0x160)
    pl = p.read32(a + 0x20)
    pos, vel, eu = v3(body + 0x60), v3(body + 0x70), v3(a + 0x5f0)
    return pos, ('t%d pl%d src%d pos %7.1f %6.1f %7.1f vel %6.1f %5.1f %6.1f |v| %5.1f yaw %5.2f gnd %d thr %.2f crash %d' % (
        p.read32(a), p.read32(pl + 0x30), p.read32(pl + 0x34), *pos, *vel, f32(a + 0x598), eu[1],
        p.read32(a + 0x590), f32(a + 0x34c), p.read32(a + 0x3b0)))


last = -1
loaded = False
while time.time() < deadline:
    try:
        f = p.read32(0x00479d04)
    except Exception:  # PINE does not answer while the VM is paused (the load, the movie's end)
        time.sleep(0.2)
        continue
    if not loaded:
        loaded = start <= f <= start + 30
        if not loaded:
            time.sleep(0.05)
            continue
    if f >= end:
        break
    if f // every == last // every:
        time.sleep(0.02)
        continue
    last = f
    try:
        (a0, s0), (a1, s1) = plane(actor(0)), plane(actor(1))
        print('%d | %s | %s | d %.1f' % (f, s0, s1, math.dist(a0, a1)), flush=True)
    except Exception as e:
        print(f, 'read failed', e, flush=True)
