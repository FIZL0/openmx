"""C14: compare two captures of the same tape row by row. A row that differs while its neighbours agree is
a torn read in one of them; show which run's row sits off the line its neighbours draw.
usage: compare.py A.csv B.csv"""
import csv, sys
a = list(csv.DictReader(open(sys.argv[1])))
b = list(csv.DictReader(open(sys.argv[2])))
skip = {'frame', 'stamp_a', 'stamp_b', 'pad_a', 'pad_b'}
assert [r['frame_counter'] for r in a] == [r['frame_counter'] for r in b], 'frame ranges differ'
bad = [i for i in range(len(a)) if any(a[i][k] != b[i][k] for k in a[i] if k not in skip)]
print(len(a), 'rows,', len(bad), 'differ')
y = lambda r: float(r['body_pos.y'])
for i in bad:
    mid = (y(a[i - 1]) + y(a[i + 1])) / 2
    off = lambda r: abs(y(r) - mid)
    print(a[i]['frame_counter'], 'prev %.4f A %.4f B %.4f next %.4f' % (y(a[i - 1]), y(a[i]), y(b[i]), y(a[i + 1])),
          'torn:', 'A' if off(a[i]) > off(b[i]) else 'B',
          'A=prev' if a[i]['body_raw'] == a[i - 1]['body_raw'] else '', 'B=prev' if b[i]['body_raw'] == b[i - 1]['body_raw'] else '',
          'A=next' if a[i]['body_raw'] == a[i + 1]['body_raw'] else '', 'B=next' if b[i]['body_raw'] == b[i + 1]['body_raw'] else '')
