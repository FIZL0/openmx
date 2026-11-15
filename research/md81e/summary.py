"""MD81e: an Untamed golf cart capture's rows, decoded. research/md164/summary.py with the gas pedal (+0x344)
and its slewed copy (+0x348) to three places, the analog throttle's words.

  python3 -I research/md81e/summary.py NAME [EVERY] [--obj OFF,OFF..]

NAME is context/capture/md81e/NAME.csv. Per row: the row, vsync step, speed (|body velocity|, ft/s),
position, velocity y, gas/brake (ctl), gear and rpm, the four tires' touch and compression, the body's
rates, the lean, preload r/a/m, timer, cue, the car's state (+0xc70), and any car words asked for (f32, hex).
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
    a = ap.parse_args()
    o_obj = [int(x, 16) for x in a.obj.split(',') if x]
    last = None
    for i, r in enumerate(csv.DictReader(open(os.path.join(ROOT, 'context', 'capture', 'md81e', a.name + '.csv')))):
        v = int(r['vsync'])
        step = '' if last is None else v - last
        last = v
        if i % a.every:
            continue
        vel = [float(r[f'vel.{c}']) for c in 'xyz']
        touch = ''.join(r[f'tire{k}.touch'] for k in range(4))
        comp = ' '.join(f"{float(r[f'tire{k}.compress']):.2f}" for k in range(4))
        rate = ' '.join(f"{float(r[f'rate.{c}']):6.2f}" for c in 'xyz')
        gear = word(r['engine'], 0xc0, '<i')
        rpm = word(r['engine'], 0x11c)
        pre = bytes.fromhex(r['preload'])
        ram = '/'.join(str(x) for x in struct.unpack('<3I', pre))
        extra = ' '.join(f'{obj(r, o):.4g}' for o in o_obj)
        print(f"{i:4d} {v} {step!s:>2} spd {math.hypot(*vel):6.2f} pos {float(r['pos.x']):8.2f} "
              f"{float(r['pos.y']):7.2f} {float(r['pos.z']):8.2f} vy {vel[1]:6.2f} "
              f"b/g {float(r['ctl.z']):.0f}/{float(r['gas.x']):.3f}/{float(r['gas.y']):.3f} g {gear} {rpm:5.0f} t {touch} c {comp} "
              f"w {rate} l {float(r['lean']):5.2f} p {ram} {float(r['pretime.y']):.3f} q {r['cued']} "
              f"s {r['carstate']} {extra}")


main()
