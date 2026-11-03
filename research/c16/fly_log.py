"""C16: log P1's plane (actor entry 0) and P2's bike live while a tape plays, one line every EVERY frames,
until frame_counter reaches END (or DEADLINE seconds). Research, not a capture: rows can tear.

usage: fly_log.py START END [EVERY] [DEADLINE]   (waits for the load: a frame within START..START+30)
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
        time.sleep(0.05)
        continue
    last = f
    try:
        a, b2 = actor(0), actor(2)
        body = p.read32(a + 0x160)
        pos, vel, eu = v3(body + 0x60), v3(body + 0x70), v3(a + 0x5f0)
        ch = p.read32(b2 + 0x7a4)
        p2 = v3(ch + 0x60)
        d = math.dist(pos, p2)
        print('%d pos %7.1f %6.1f %7.1f vel %6.1f %5.1f %6.1f |v| %5.1f eu %5.2f %5.2f %5.2f gnd %d thr %.2f '
              'crash %d %.2f p2 %7.1f %6.1f %7.1f d %.0f' % (
                  f, *pos, *vel, f32(a + 0x598), *eu, p.read32(a + 0x590), f32(a + 0x34c),
                  p.read32(a + 0x3b0), f32(a + 0x3bc), *p2, d), flush=True)
    except Exception as e:
        print(f, 'read failed', e, flush=True)
