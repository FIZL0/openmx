"""MD80a: the body's velocity in its own frame and its rotation rate, for find.py's EXPR.

  python3 -I research/md80a/find.py CSV "__import__('frames').local(c, K)" ...

is awkward, so this prints instead, for one csv, which words hold each component of the local velocity
(conj(q) v q), of the world-frame angular velocity (q w q*, w = body +0x1d0) and of atan2 angles of the local
velocity, over every 37th row.
  python3 -I research/md80a/frames.py CSV
"""
import csv
import math
import struct
import sys

csv.field_size_limit(1 << 26)


def rot(q, v, inv=False):
    x, y, z, w = q
    if inv:
        x, y, z = -x, -y, -z
    tx, ty, tz = 2 * (y * v[2] - z * v[1]), 2 * (z * v[0] - x * v[2]), 2 * (x * v[1] - y * v[0])
    return (v[0] + w * tx + (y * tz - z * ty), v[1] + w * ty + (z * tx - x * tz), v[2] + w * tz + (x * ty - y * tx))


def main():
    rows = list(csv.DictReader(open(sys.argv[1])))
    blocks = ['obj', 'body', 'engine', 'rotor', 'aero']
    picks = range(5, len(rows), 37)
    hits = {}
    for i in picks:
        r = rows[i]
        cache = {k: bytes.fromhex(r['obj0'] + r['obj1']) if k == 'obj' else bytes.fromhex(r[k]) for k in blocks if k == 'obj' or k in r}
        q = [float(r[f'quat.{c}']) for c in 'xyzw']
        v = [float(r[f'vel.{c}']) for c in 'xyz']
        wb = struct.unpack_from('<3f', cache['body'], 0x1d0)
        lv = rot(q, v, inv=True)
        ww = rot(q, wb)
        want = {f'local_vel.{"xyz"[k]}': lv[k] for k in range(3)}
        want.update({f'world_rate.{"xyz"[k]}': ww[k] for k in range(3)})
        want['atan2(-lv.y, lv.z)'] = math.atan2(-lv[1], lv[2])
        want['atan2(lv.y, lv.z)'] = math.atan2(lv[1], lv[2])
        want['atan2(lv.x, lv.z)'] = math.atan2(lv[0], lv[2])
        for name, x in want.items():
            found = set()
            for k, b in cache.items():
                for o, (y,) in enumerate(struct.iter_unpack('<f', b[:len(b) // 4 * 4])):
                    if abs(y - x) <= 2e-3 * max(1.0, abs(x)):
                        found.add((k, o * 4))
            hits[name] = found if name not in hits else hits[name] & found
    for name, found in hits.items():
        print(name, ' '.join(f'{k}+{o:#x}' for k, o in sorted(found)))


main()
