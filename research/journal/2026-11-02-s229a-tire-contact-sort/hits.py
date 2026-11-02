"""S229a: the contact sort's hit list on chosen rows of a capture: per hit, the ray pair (list +0x90), the
normal (+0x94), the point (+0x98) and the hit object (+0x9c); and each tire's sorted words.

usage: hits.py CSV FRAME_COUNTER...
"""
import csv
import struct
import sys

want = {int(a) for a in sys.argv[2:]}
for x in csv.DictReader(open(sys.argv[1])):
    if int(x['frame_counter']) not in want:
        continue
    lst = bytes.fromhex(x['sort_list_raw'])
    n = struct.unpack_from('<i', lst, 0xc)[0]
    rays, normals, points = (bytes.fromhex(x[k]) for k in ('sort_rays_raw', 'sort_normals_raw', 'sort_points_raw'))
    hits = bytes.fromhex(x['sort_hits_raw'])
    print('fc', x['frame_counter'], 'hits', n, 'pos', [x['body_pos.' + k] for k in 'xyz'])
    v = lambda b, o: ' '.join(f'{f:9.4f}' for f in struct.unpack_from('<4f', b, o))
    for i in range(min(n, 8)):
        print(f'  {i} ray {v(rays, 32 * i)} | {v(rays, 32 * i + 16)}  n {v(normals, 16 * i)}  p {v(points, 16 * i)}'
              f'  hit {struct.unpack_from("<I", hits, 4 * i)[0]:#x}')
    for t in ('fl', 'fr', 'rl', 'rr'):
        h = bytes.fromhex(x[f'tire_{t}_raw'])
        w = lambda o: struct.unpack_from('<I', h, o)[0]
        print(f'  {t} b4 {w(0xb4)} b8 {w(0xb8)} bc {w(0xbc)} 1ac {w(0x1ac):#x} 1b0 {w(0x1b0):#x} 1c8 {w(0x1c8):#x}'
              f'  p {v(h, 0x110)}  n {v(h, 0x120)}  drop {v(h, 0x70)}')
