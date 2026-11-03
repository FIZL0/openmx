"""MD39a: a car capture's rows, decoded.

  python3 -I research/md39a/summary.py NAME [EVERY] [--obj OFF,OFF..] [--tire OFF,..] [--engine OFF,..]

NAME is context/capture/md39a/NAME.csv. Per row: the row, vsync step, speed (|body velocity|, ft/s), position, velocity y, the four tires' touch and
compression, and any car-object / tire / engine words asked for (f32, offsets in hex).
"""
import argparse
import csv
import math
import os
import struct

csv.field_size_limit(1 << 26)
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def word(hexs, off, fmt='<f'):
    return struct.unpack_from(fmt, bytes.fromhex(hexs[2 * off:2 * off + 8]))[0]


def obj(row, off):
    return word(row['obj0'], off) if off < 0xc00 else word(row['obj1'], off - 0xc00)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('name')
    ap.add_argument('every', nargs='?', type=int, default=1)
    ap.add_argument('--obj', default='')
    ap.add_argument('--tire', default='')
    ap.add_argument('--engine', default='')
    a = ap.parse_args()
    offs = lambda s: [int(x, 16) for x in s.split(',') if x]
    o_obj, o_tire, o_eng = offs(a.obj), offs(a.tire), offs(a.engine)
    last = None
    for i, r in enumerate(csv.DictReader(open(os.path.join(ROOT, 'context', 'capture', 'md39a', a.name + '.csv')))):
        v = int(r['vsync'])
        step = '' if last is None else v - last
        last = v
        if i % a.every:
            continue
        vel = [float(r[f'vel.{c}']) for c in 'xyz']
        touch = ''.join(r[f'tire{k}.touch'] for k in range(4))
        comp = ' '.join(f"{float(r[f'tire{k}.compress']):.2f}" for k in range(4))
        extra = ' '.join(f'{obj(r, o):.4g}' for o in o_obj)
        extra += ' | ' + ' '.join(f"{word(r[f'tire{k}.raw'], o):.4g}" for o in o_tire for k in range(4)) if o_tire else ''
        extra += ' | ' + ' '.join(f"{word(r['engine'], o):.4g}" for o in o_eng) if o_eng else ''
        print(f"{i:4d} {v} {step!s:>2} spd {math.hypot(*vel):6.2f} pos {float(r['pos.x']):8.2f} "
              f"{float(r['pos.y']):7.2f} {float(r['pos.z']):8.2f} vy {vel[1]:6.2f} t {touch} c {comp} {extra}")


main()
