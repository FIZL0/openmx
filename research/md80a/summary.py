"""MD80a: a flier capture's rows, decoded.

  python3 -I research/md80a/summary.py NAME [EVERY] [--obj OFF,..] [--body OFF,..] [--a OFF,..] [--b OFF,..]

NAME is context/capture/md80a/NAME.csv. Per row: the row, vsync step, wreck word, speed (|body velocity|,
ft/s), pitch and bank (degrees, the body's +z and +x axes against level), position, velocity y, the input words (ctl.z/w = +0x310/+0x314, inp = +0x10f0..), and any words asked
for (f32, offsets in hex): --obj the flier object, --body the rigid body, --a the engine (biplane) or rotor
(Kiowa), --b the aero.
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


def rot(q, v):
    """v turned by the quaternion q = (x, y, z, w)."""
    x, y, z, w = q
    tx, ty, tz = 2 * (y * v[2] - z * v[1]), 2 * (z * v[0] - x * v[2]), 2 * (x * v[1] - y * v[0])
    return (v[0] + w * tx + (y * tz - z * ty), v[1] + w * ty + (z * tx - x * tz), v[2] + w * tz + (x * ty - y * tx))


def attitude(r):
    """Pitch and bank in degrees: asin of the body's forward (+z) and right (+x) axes' world y."""
    q = [float(r[f'quat.{c}']) for c in 'xyzw']
    return tuple(math.degrees(math.asin(max(-1.0, min(1.0, rot(q, v)[1])))) for v in ((0, 0, 1), (1, 0, 0)))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('name')
    ap.add_argument('every', nargs='?', type=int, default=1)
    ap.add_argument('--obj', default='')
    ap.add_argument('--body', default='')
    ap.add_argument('--a', default='')
    ap.add_argument('--b', default='')
    a = ap.parse_args()
    offs = lambda s: [int(x, 16) for x in s.split(',') if x]
    o_obj, o_body, o_a, o_b = offs(a.obj), offs(a.body), offs(a.a), offs(a.b)
    last = None
    for i, r in enumerate(csv.DictReader(open(os.path.join(ROOT, 'context', 'capture', 'md80a', a.name + '.csv')))):
        v = int(r['vsync'])
        step = '' if last is None else v - last
        last = v
        if i % a.every:
            continue
        kiowa = 'rotor' in r
        vel = [float(r[f'vel.{c}']) for c in 'xyz']
        inp = [float(r[f'inp.{c}']) for c in 'xyzw'] + ([float(r[f'inp2.{c}']) for c in 'xy'] if kiowa else [])
        sub_a = 'rotor' if kiowa else 'engine'
        extra = ' '.join(f'{obj(r, o):.4g}' for o in o_obj)
        extra += ' | ' + ' '.join(f"{word(r['body'], o):.4g}" for o in o_body) if o_body else ''
        extra += ' | ' + ' '.join(f"{word(r[sub_a], o):.4g}" for o in o_a) if o_a else ''
        extra += ' | ' + ' '.join(f"{word(r['aero'], o):.4g}" for o in o_b) if o_b else ''
        pitch, bank = attitude(r)
        print(f"{i:4d} {step!s:>2} w{r['wreck']} p{pitch:4.0f} b{bank:4.0f} spd {math.hypot(*vel):6.2f} pos {float(r['pos.x']):8.2f} "
              f"{float(r['pos.y']):7.2f} {float(r['pos.z']):8.2f} vy {vel[1]:6.2f} "
              f"ctl {float(r['ctl.z']):.2g} {float(r['ctl.w']):.2g} inp {' '.join(f'{x:.3g}' for x in inp)} {extra}")


main()
