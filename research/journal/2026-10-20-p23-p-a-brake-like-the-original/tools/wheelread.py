"""Per tick, both wheels: load +0xf8, forward force +0x858, flags +0xe0/+0xe4, spin +0x1d4, lever *(+0x30),
contact +0xdc, and the bike's velocity. usage: wheelread.py FIRST LAST"""
import struct, sys, time, math
sys.path.insert(0, '/home/rycs/repos/openmx-slots/s1')
from ps2kit.pine import Pine
lo, hi = int(sys.argv[1]), int(sys.argv[2])
p = Pine()
FC = 0x479d04
W = (0xda7d10, 0xda26d0)
BIKE = 0xdad810
last = None
t0 = time.time()
rf = lambda a: struct.unpack('<f', struct.pack('<I', p.read32(a)))[0]
while time.time() - t0 < 240:
    f = p.read32(FC)
    if f == last:
        time.sleep(0.002)
        continue
    last = f
    if f < lo or f > hi:
        if hi < f < hi + 1000:
            break
        continue
    v = struct.unpack("<3f", p.read_block(0xda79d0 + 0x260, 12))
    out = [f, round(math.hypot(v[0], v[2]), 2)]
    for w in W:
        blk = p.read_block(w + 0xd0, 0x30)
        dc, e0, e4 = struct.unpack('<I', blk[0xc:0x10])[0], *struct.unpack('<II', blk[0x10:0x18])
        load = struct.unpack('<f', blk[0x28:0x2c])[0]
        fwd = rf(w + 0x858)
        side = rf(w + 0x850)
        spin = rf(w + 0x1d4)
        lever = rf(p.read32(w + 0x30))
        out += ['|', dc, e0, e4, round(load, 1), round(fwd, 1), round(side, 1), round(spin, 2), round(lever, 2)]
    print(*out, flush=True)
