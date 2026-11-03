"""MD80a: which words of a raw block are the angular velocity: the quaternion's own rate (central difference
over two rows, 2/30 s), world and body frame, against every f32 triple in the block.

  python3 -I research/md80a/rate.py CSV [BLOCK] [ROWS]
"""
import csv, math, struct, sys
csv.field_size_limit(1 << 26)
rows = list(csv.DictReader(open(sys.argv[1])))
blk = sys.argv[2] if len(sys.argv) > 2 else 'body'
picks = [int(x) for x in sys.argv[3].split(',')] if len(sys.argv) > 3 else range(200, len(rows) - 2, 97)


def q(r):
    return [float(r[f'quat.{c}']) for c in 'xyzw']


def mul(a, b):
    ax, ay, az, aw = a
    bx, by, bz, bw = b
    return [aw * bx + ax * bw + ay * bz - az * by, aw * by - ax * bz + ay * bw + az * bx,
            aw * bz + ax * by - ay * bx + az * bw, aw * bw - ax * bx - ay * by - az * bz]


def conj(a):
    return [-a[0], -a[1], -a[2], a[3]]


def words(r):
    b = bytes.fromhex(r[blk])
    return struct.unpack(f'<{len(b) // 4}f', b)


score = {}
for i in picks:
    q0, q1 = q(rows[i - 1]), q(rows[i + 1])
    dt = 2 / 30
    dq = [(b - a) / dt for a, b in zip(q0, q1)]
    qm = q(rows[i])
    ww = mul(dq, conj(qm))[:3]   # world rate / 2
    wb = mul(conj(qm), dq)[:3]   # body rate / 2
    ww = [2 * x for x in ww]
    wb = [2 * x for x in wb]
    w = words(rows[i])
    for o in range(0, len(w) - 2):
        for name, ref in (('world', ww), ('body', wb)):
            n = math.sqrt(sum(x * x for x in ref)) + 1e-3
            err = math.sqrt(sum((w[o + k] - ref[k]) ** 2 for k in range(3))) / n
            score.setdefault((o * 4, name), []).append(err)
best = sorted((sum(v) / len(v), k) for k, v in score.items() if all(math.isfinite(x) for x in v))[:6]
for e, (o, name) in best:
    print(f'{blk} +{o:#x} {name}: mean rel err {e:.3g}')
i = picks[0]
print('sample row', i, 'ref world', [round(x, 4) for x in ww])
