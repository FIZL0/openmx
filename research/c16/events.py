"""C16: the events on the flying tape's capture: plane 0's take-offs and touch-downs (on_ground), its crash
word, its sharp speed losses (a contact: |body_vel| drops by more than DROP ft/s over 6 ticks), P2's
wreck reason and body mode, and the rows where plane 0 comes within 15 ft of P2's chassis centre.

usage: python3 -B research/c16/events.py [context/capture/c16/fly.csv] [DROP]
"""
import csv
import math
import sys

path = sys.argv[1] if len(sys.argv) > 1 else 'context/capture/c16/fly.csv'
drop = float(sys.argv[2]) if len(sys.argv) > 2 else 8.0
rows = list(csv.DictReader(open(path)))
v3 = lambda r, n: tuple(float(r[n + '.' + k]) for k in 'xyz')
fr = lambda r: int(r['frame_counter'])


def where(r):
    p, v = v3(r, 'body_pos'), v3(r, 'body_vel')
    return 'at (%.1f, %.1f, %.1f) |v| %.1f speed %.1f' % (*p, math.hypot(*v), float(r['speed']))


print('%d rows, frames %d..%d; gaps: %s' % (len(rows), fr(rows[0]), fr(rows[-1]),
      [fr(b) for a, b in zip(rows, rows[1:]) if fr(b) != fr(a) + 1][:10] or 'none'))
print('plane_ctl_src first/last:', rows[0]['plane_ctl_src'], rows[-1]['plane_ctl_src'])
near = False
for i in range(1, len(rows)):
    a, r = rows[i - 1], rows[i]
    for col, what in (('on_ground', 'plane 0 on_ground'), ('crash', 'plane 0 crash'),
                      ('p2_wreck_reason', 'P2 wreck reason'), ('p2_mode', 'P2 body mode'),
                      ('plane1_crash', 'plane 1 crash')):
        if a[col] != r[col]:
            print(fr(r), '%s %s -> %s' % (what, a[col], r[col]), where(r))
    if i >= 6:
        s0, s1 = math.hypot(*v3(rows[i - 6], 'body_vel')), math.hypot(*v3(r, 'body_vel'))
        s_prev = math.hypot(*v3(rows[i - 7], 'body_vel')) if i >= 7 else s0
        if s0 - s1 > drop and not s_prev - math.hypot(*v3(a, 'body_vel')) > drop:
            print(fr(r), 'plane 0 speed loss %.1f -> %.1f over 6 ticks' % (s0, s1), where(r))
    d = math.dist(v3(r, 'body_pos'), v3(r, 'p2_ch_centre'))
    if (d < 15) != near:
        near = d < 15
        print(fr(r), 'plane 0 %s 15 ft of P2 (%.1f ft)' % ('within' if near else 'out of', d), where(r))
