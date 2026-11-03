"""C18: one capture's summary: frames, the wreck words, and each entry's flags, switch and carry moves.

usage: python3 -B summary.py NAME   (context/capture/c18/NAME.csv)
"""
import csv
import sys

r = list(csv.DictReader(open(f'context/capture/c18/{sys.argv[1]}' + '.csv')))
n = sum(1 for k in r[0] if k.endswith('_flag') and k.startswith('e'))
fc = [int(x['frame_counter']) for x in r]
print(len(r), 'rows', fc[0], '..', fc[-1], 'kind', sorted(set(x['car_kind'] for x in r)), 'steps', sorted(set(b - a for a, b in zip(fc, fc[1:]))))
for k in ['wreck_reason', 'wreck_lifted', 'w754', 'feed_wreck']:
    vals = [x[k] for x in r]
    ch = [(fc[i], v) for i, v in enumerate(vals) if i == 0 or v != vals[i - 1]]
    print(k, ch[:12])
print('speed max', max(float(x['speed']) for x in r), 'throttle', sorted(set(x['throttle'] for x in r))[:6])
for i in range(n):
    fl = sorted(set(x[f'e{i}_flag'] for x in r))
    on = sorted(set(int(x[f'e{i}_on']) & 4 for x in r))
    moves = sum(1 for a, b in zip(r, r[1:]) if a[f'e{i}_carry'] != b[f'e{i}_carry'])
    print(f'e{i}', 'flags', fl, 'on', on, 'carry moves', moves)
