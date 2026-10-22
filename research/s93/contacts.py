"""Summarise a split capture (S93): continuity, guards, and every tick with a rider-rider contact.

    python3 -B research/s93/contacts.py split/splitscreen2      # context/capture/<name>.csv

A contact is any entry of either record's list whose +0x74 (other) is not 0: the other side is a
collision record, i.e. the other rider. Prints runs of contact ticks with the entry kinds, the
rays' heights (end y; the head ray reaches 6.67), the normal, both riders' speeds and wreck reasons.
"""
import csv, math, os, sys

rows = list(csv.DictReader(open(os.path.join('context', 'capture', sys.argv[1] + '.csv'))))
fc = [int(r['frame_counter']) for r in rows]
gaps = [(a, b) for a, b in zip(fc, fc[1:]) if b != a + 1]
print('%d rows, frames %d..%d, gaps %s' % (len(rows), fc[0], fc[-1], gaps[:10]))
bad = [r['frame_counter'] for r in rows if (r['p1_type'], r['p1_index'], r['p2_type'], r['p2_index'],
       r['p1_bike_type'], r['p1_bike_player'], r['p2_bike_type'], r['p2_bike_player']) !=
       ('2', '0', '2', '1', '4', '0', '4', '1')]
print('guard misses: %d %s' % (len(bad), bad[:10]))
print('dt values: %s' % sorted({r['dt'] for r in rows}))


def v(r, name):
    return tuple(float(r[name + '.' + c]) for c in 'xyz')


def speed(r, p):
    return math.sqrt(sum(x * x for x in v(r, 'p%d_ch_vel' % p)))


def pos(r, p):
    return tuple(float(r['p%d_bike_xform.m3%d' % (p, k)]) if 'p%d_bike_xform.m30' % p in r else 0 for k in range(3))


prev = {}
sig_prev = {1: None, 2: None}
for r in rows:
    t = int(r['frame_counter'])
    out = []
    for p in (1, 2):
        n = int(r['p%d_rec_count' % p])
        hits = [k for k in range(min(n, 11)) if int(r['p%d_c%d.other' % (p, k)]) != 0]
        # a wrecked rider's record is not refreshed, so print a list only when it changes
        sig = tuple(r['p%d_c%d.%s' % (p, k, f)] for k in hits for f in ('kind', 'normal.x', 'end.y'))
        if hits and sig != sig_prev[p]:
            kinds = ''.join(r['p%d_c%d.kind' % (p, k)] for k in hits)
            ys = max(float(r['p%d_c%d.end.y' % (p, k)]) for k in hits)
            out.append('p%d %d actor hits kinds %s top ray %.2f n (%.2f %.2f %.2f)' % (
                p, len(hits), kinds, ys, *v(r, 'p%d_c%d.normal' % (p, hits[0]))))
        sig_prev[p] = sig
    w = (r['p1_wreck_reason'], r['p1_wreck_state'], r['p2_wreck_reason'], r['p2_wreck_state'])
    if out or w != prev.get('w'):
        print('%d  speed %.1f/%.1f  wreck p1 %s/%s p2 %s/%s  %s' % (t, speed(r, 1), speed(r, 2), *w, '; '.join(out)))
    prev['w'] = w
