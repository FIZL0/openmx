"""C43: the rows around the strike. Per row: the helicopter's velocity and rates, its wreck words (+0x3d4
reason, +0x3d8 wrecked), its record's entries (kind word, and every word that points into the prop's
objects 0x00e2fd40..0x00e3e000), and the prop's enabled, velocity, rates, collection flags, f.col +0x116
and its record's entries the same way.

usage: python3 -I research/c43/strike_rows.py CSV FIRST LAST
"""
import csv
import struct
import sys

FIRST, LAST = int(sys.argv[2]), int(sys.argv[3])
PROP = range(0x00e2fd40, 0x00e3e000)


def raw(s):
    return bytes.fromhex(s.replace(' ', ''))


def v(r, k, n=3):
    return ' '.join('%8.3f' % float(r[k + '.' + c]) for c in 'xyzw'[:n])


def entries(blob, count, size=0x80):
    """Each entry: kind, the other's collision object (+0x74), the float +0x24, and +0x8 when set."""
    out = []
    for i in range(min(count, len(blob) // size)):
        w = struct.unpack_from('<32I', blob, i * size)
        f = struct.unpack_from('<32f', blob, i * size)
        out.append('k%d other %#x +0x24 %.3f%s' % (w[0], w[0x74 // 4], f[9], ' +0x8 %#x' % w[2] if w[2] else ''))
    return out


for r in csv.DictReader(open(sys.argv[1])):
    f = int(r['frame_counter'])
    if not FIRST <= f <= LAST:
        continue
    a = raw(r['heli_a'])
    reason, wrecked = struct.unpack_from('<II', a, 0x3d4 - 0x330)
    print(f, 'heli v', v(r, 'body_vel'), 'w', v(r, 'body_rate'), 'reason', reason, 'wrecked', wrecked)
    for e in entries(raw(r['rec_entries']), int(r['rec_count'])):
        print('     heli', e)
    fc = raw(r['prop_fcol'])
    print('     prop on', r['prop_enabled'], 'v', v(r, 'prop_vel'), 'w', v(r, 'prop_rate'),
          'coll %#x' % int(r['prop_coll_flags']), 'fcol+0x116 %#x' % fc[0x16])
    for e in entries(raw(r['prop_rec_entries']), int(r['prop_rec_count'])):
        print('     prop', e)
