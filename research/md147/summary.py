"""MD147: an Untamed race quad capture's rows, decoded (research/md147/quad.toml).

  python3 -I research/md147/summary.py NAME [--every N] [--from R] [--to R] [--obj OFF ...] [--engine OFF ...]

Per row: vsync, pos, speed, euler, the controls (brake, gas), the stick columns, the wreck word, the hazard
words (stuck, free) and clock, and each wheel's grounded (+0xd0) and touch (+0xdc). --obj/--engine add a
word of the quad / engine raw block (hex offset; printed as f32 and hex).
"""
import argparse
import csv
import os
import struct

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..')


def word(hexs, off, fmt='<f'):
    return struct.unpack_from(fmt, bytes.fromhex(hexs[2 * off:2 * off + 8]))[0]


def obj(row, off):
    return word(row['obj0'], off, '<I') if off < 0xc00 else word(row['obj1'], off - 0xc00, '<I')


def show(bits):
    x = struct.unpack('<f', struct.pack('<I', bits))[0]
    return f'{x:.4g}/{bits:x}'


def main():
    p = argparse.ArgumentParser()
    p.add_argument('name')
    p.add_argument('--every', type=int, default=10)
    p.add_argument('--from', dest='lo', type=int, default=0)
    p.add_argument('--to', dest='hi', type=int, default=10 ** 9)
    p.add_argument('--obj', nargs='*', default=[])
    p.add_argument('--engine', nargs='*', default=[])
    a = p.parse_args()
    csv.field_size_limit(1 << 24)
    path = os.path.join(ROOT, 'context/capture/md147', a.name + '.csv')
    g = lambda r, k: float(r[k])
    for i, r in enumerate(csv.DictReader(open(path))):
        if i < a.lo or i > a.hi or (i - a.lo) % a.every:
            continue
        hz = bytes.fromhex(r['hazard'])
        stuck, free = struct.unpack_from('<II', hz)
        wheels = ''.join(f"{r[f'wheel{w}.grounded']}{r[f'wheel{w}.touch']} " for w in range(4))
        line = (f"{i:4d} v{r['vsync']} pos ({g(r, 'pos.x'):7.2f} {g(r, 'pos.y'):6.2f} {g(r, 'pos.z'):7.2f}) "
                f"spd {g(r, 'speed'):6.2f} eul ({g(r, 'euler.x'):+.3f} {g(r, 'euler.y'):+.3f} {g(r, 'euler.z'):+.3f}) "
                f"bg ({g(r, 'ctl.z'):.2f} {g(r, 'ctl.w'):.2f}) st ({g(r, 'stick.x'):+.2f} {g(r, 'stick.y'):+.2f}) "
                f"sl ({g(r, 'stick_lim.x'):+.2f} {g(r, 'stick_lim.y'):+.2f}) wr {r['wreck']} gr {r['ground']} "
                f"hz {stuck}{free} t {g(r, 'stuck_time'):.3f} w {wheels}")
        for o in a.obj:
            line += f' q+{o} {show(obj(r, int(o, 16)))}'
        for o in a.engine:
            line += f" e+{o} {show(word(r['engine'], int(o, 16), '<I'))}"
        print(line)


main()
