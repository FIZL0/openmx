"""S228: summarise the recon capture: list changes, plane motion and wreck words, riders' wreck reasons.
usage: python3 -B research/s228/summary.py context/capture/s228/recon.csv | head"""
import csv, sys, math
rows = list(csv.DictReader(open(sys.argv[1])))
prev = None
for r in rows:
    key = tuple((r[f'e{k}_type'], r[f'e{k}_actor'], r[f'e{k}_player']) for k in range(8))
    if key != prev:
        print(r['frame_counter'], 'list', [(t, hex(int(a)) if a else '', hex(int(p)) if p else '') for t, a, p in key if t])
    prev = key
def f(r, k):
    try: return float(r[k])
    except: return float('nan')
last = {}
for r in rows:
    fc = int(r['frame_counter'])
    for k in ('p1_wreck_reason', 'p2_wreck_reason', 'pl0_wreck', 'pl1_wreck', 'pl0_flown', 'pl1_flown', 'p1_type', 'p2_type'):
        if last.get(k) != r[k]:
            print(fc, k, last.get(k), '->', r[k])
            last[k] = r[k]
    if fc % 120 == 0:
        print(fc, 'p1 %.0f %.0f %.0f v%.1f' % (f(r,'p1_centre.x'), f(r,'p1_centre.y'), f(r,'p1_centre.z'), f(r,'p1_speed')),
              'p2 v%.1f' % f(r,'p2_speed'),
              'pl0 %.0f %.0f %.0f v%.1f' % (f(r,'pl0_pos.x'), f(r,'pl0_pos.y'), f(r,'pl0_pos.z'), f(r,'pl0_speed')),
              'pl1 %.0f %.0f %.0f v%.1f' % (f(r,'pl1_pos.x'), f(r,'pl1_pos.y'), f(r,'pl1_pos.z'), f(r,'pl1_speed')), 'pad', r['pad_a'])
