"""S291: print a gate.py capture around its teleport. usage: python3 -I research/s291/show.py CSV [K...]"""
import csv
import struct
import sys

rows = list(csv.DictReader(open(sys.argv[1])))
ks = [int(k) for k in sys.argv[2:]] or [0]


def f(h):
    return struct.unpack('<f', struct.pack('<I', int(h, 16)))[0]


def q(h):
    return '(' + ' '.join('%.4f' % f(x) for x in h.split(':')[:3]) + ')'


frames = [int(r['frame']) for r in rows if r['phase'] == 'run']
gaps = [(a, b) for a, b in zip(frames, frames[1:]) if b != a + 1]
print('rows', len(rows), 'frames', frames[0], '..', frames[-1], 'gaps', gaps)
at = next(i for i, r in enumerate(rows) if r['phase'] == 'pause')
for r in rows[at - 3:at + 14]:
    print(r['phase'], r['frame'], r['rng'], 'gate', r['gate_flags'], 'el %.4f' % f(r['elapsed']), 'order', r['order'])
    for k in ks:
        p = 'k%d_' % k
        print('   k%d now' % k, q(r[p + 'now']), 'last', q(r[p + 'last']), 'pen %.4f' % f(r[p + 'pen']), 'pos', q(r[p + 'pos']),
              'vel', q(r[p + 'vel']), 'free', r[p + 'free'].replace('3f800000', '1').replace('00000000', '0'))
