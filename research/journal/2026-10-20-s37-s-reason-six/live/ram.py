"""S37: the bike, its collision record and the record's contact list, read out of an EE RAM image.

    python3 -I ram.py context/states/s37-9.ee.bin [capture.csv]

With a capture, also says which of its rows the image's bike matches (by position), which is how a
state is placed on a tick rather than trusted by its frame_counter.
"""
import csv
import struct
import sys

ram = open(sys.argv[1], 'rb').read()


def u32(a):
    return struct.unpack_from('<I', ram, a & 0x1ffffff)[0]


def f32(a, n=1):
    v = struct.unpack_from(f'<{n}f', ram, a & 0x1ffffff)
    return v if n > 1 else v[0]


GAME_MANAGER, FRAME_COUNTER = 0x4e1e98, 0x479d04
a = u32(GAME_MANAGER)
for off in (0x34, 0x34, 0x8, 0x8):
    a = u32(a + off)
bike = u32(a)
rec = u32(bike + 0x7d0)
chassis = u32(bike + 0x7a4)
print(f'frame_counter {u32(FRAME_COUNTER)}  bike {bike:#x} type {u32(bike)}  reason {u32(bike + 0x748)}'
      f'  +0x588 {u32(bike + 0x588)}  record {rec:#x}  chassis {chassis:#x}')
print(f'chassis pos {f32(chassis + 0x60, 4)}  vel {f32(chassis + 0x260, 4)}  mass {f32(chassis + 0x210)}')
print(f'record +0x40 {u32(rec + 0x40)}  count {u32(rec + 0x100)}  capacity {u32(rec + 0x104)}'
      f'  list {u32(rec + 0x108):#x}  +0x34 {u32(rec + 0x34)}  +0x1f0 {u32(rec + 0x1f0):#x}')
lst = u32(rec + 0x108)
for i in range(u32(rec + 0x100)):
    e = lst + i * 0x80
    print(f'  [{i}] kind {u32(e)} from {f32(e + 0x10, 3)} to {f32(e + 0x20, 3)} other {u32(e + 0x74):#x}')
# The bike's own rays: the collision object at +0x1f0, its +0x168 count and +0x170 (start, end) pairs.
obj = u32(rec + 0x1f0)
if obj:
    n, rays = u32(obj + 0x168), u32(obj + 0x170)
    print(f'rays: {n}')
    for i in range(min(n, 64)):
        print(f'  ray {i}: {f32(rays + i * 0x20, 3)} -> {f32(rays + i * 0x20 + 0x10, 3)}')
# The capture's bike columns are the vehicle actor's, eight list hops on (watch/acceptance.toml).
v = u32(GAME_MANAGER)
for off in (0x34, 0x34) + (0x8,) * 8:
    v = u32(v + off)
v = u32(v)
xform = f32(v + 0x180, 16)
print('vehicle +0x1b0 position', xform[12:15])
if len(sys.argv) > 2:
    rows = list(csv.DictReader(open(sys.argv[2])))
    pos = xform[12:15]
    for r in rows:
        p = tuple(float(r[f'bike_xform.m3{k}']) for k in range(3)) if 'bike_xform.m30' in r else None
        if p is None:
            print('capture columns:', [k for k in r if k.startswith('bike_xform')][:16])
            break
        if all(abs(x - y) < 1e-4 for x, y in zip(p, pos)):
            print('matches capture row frame_counter', r['frame_counter'])
