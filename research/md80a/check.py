"""MD80a: the four flier captures and their RAM images exist and read as the journal says.

  python3 -I research/md80a/check.py

For each flight: the image (context/mxvatv-ps2/md80a/NAME.p2s, unzipped by ram.py to NAME.ee.bin) has the
player's flier at OBJ with the flier's vtable at +4 and the model word expected; the capture
(context/capture/md80a/NAME.csv) has ROWS rows, the vtable on every row, vsync +2 a row, the wreck word 0 on
the last row, the decoded pos/vel/quat equal to the raw body block's +0x60/+0x80/+0x170, and the body rate
(+0x1d0) against the quaternion's derivative. Rows whose body is the row before's again (a stale read,
research/journal 2026-11-03-md80a) are counted and listed. Exits 1 on any failure.
"""
import csv
import math
import os
import struct
import sys
import zipfile

csv.field_size_limit(1 << 26)
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
# name: (vtable, model word offset, model word, player object, rows, first vsync)
FLIGHTS = {
    'biplane': (0x3ce7b0, 0x1320, 1, 0x014de810, 1625, 27581),
    'biplane3': (0x3ce7b0, 0x1320, 0, 0x01b13790, 2338, 30001),
    'kiowa': (0x3cfad0, 0x1340, 1, 0x01ab7d20, 1265, 27761),
    'kiowa3': (0x3cfad0, 0x1340, 0, 0x014c8f00, 1380, 30183),
}
bad = []


def need(ok, what):
    if not ok:
        bad.append(what)
        print('  FAIL', what)


def qmul(a, b):
    ax, ay, az, aw = a
    bx, by, bz, bw = b
    return [aw * bx + ax * bw + ay * bz - az * by, aw * by - ax * bz + ay * bw + az * bx,
            aw * bz + ax * by - ay * bx + az * bw, aw * bw - ax * bx - ay * by - az * bz]


for name, (vt, moff, model, obj, nrows, first) in FLIGHTS.items():
    print(name)
    with zipfile.ZipFile(os.path.join(ROOT, 'context', 'mxvatv-ps2', 'md80a', name + '.p2s')) as z:
        ram = z.read(next(n for n in z.namelist() if n.endswith('eeMemory.bin')))
    u = lambda a: struct.unpack_from('<I', ram, a)[0]
    need(u(obj + 4) == vt, f'{name}: image vtable at {obj:#x}+4')
    need(u(obj + moff) == model, f'{name}: image model word {model}')
    rows = list(csv.DictReader(open(os.path.join(ROOT, 'context', 'capture', 'md80a', name + '.csv'))))
    need(len(rows) == nrows, f'{name}: {len(rows)} rows, want {nrows}')
    need(int(rows[0]['vsync']) == first, f'{name}: first vsync {rows[0]["vsync"]}, want {first}')
    need(all(int(r['vptr']) == vt for r in rows), f'{name}: vtable on every row')
    steps = [int(b['vsync']) - int(a['vsync']) for a, b in zip(rows, rows[1:])]
    need(all(s == 2 for s in steps), f'{name}: vsync +2 a row ({sorted(set(steps))})')
    need(int(rows[-1]['wreck']) == 0, f'{name}: wreck word 0 at the end')
    stale, err = [], []
    for i, r in enumerate(rows):
        b = bytes.fromhex(r['body'])
        for col, off in (('pos', 0x60), ('vel', 0x80), ('quat', 0x170)):
            got = struct.unpack_from('<4f', b, off)
            if any(float(r[f'{col}.{c}']) != g for c, g in zip('xyzw', got)):
                need(False, f'{name}: row {i} {col} is not body +{off:#x}')
        if i and r['body'] == rows[i - 1]['body']:
            stale.append(i)
        if 0 < i < len(rows) - 1 and i - 1 not in stale and i not in stale and i + 1 not in stale:
            q = lambda k: [float(rows[k][f'quat.{c}']) for c in 'xyzw']
            dq = [(y - x) * 15 for x, y in zip(q(i - 1), q(i + 1))]
            ref = [2 * x for x in qmul([-x for x in q(i)[:3]] + [q(i)[3]], dq)[:3]]
            w = struct.unpack_from('<3f', b, 0x1d0)
            n = math.sqrt(sum(x * x for x in ref))
            if n > 0.05:
                err.append(math.sqrt(sum((x - y) ** 2 for x, y in zip(w, ref))) / n)
    need(len(stale) <= 2, f'{name}: {len(stale)} stale rows')
    mean = sum(err) / len(err)
    need(mean < 0.05, f'{name}: body +0x1d0 against the quaternion rate, mean rel err {mean:.3f}')
    print(f'  {len(rows)} rows, vsync {rows[0]["vsync"]}..{rows[-1]["vsync"]}, stale rows {stale}, '
          f'rate err {mean:.3f} over {len(err)} rows, image ok')
print('FAIL' if bad else 'ok')
sys.exit(1 if bad else 0)
