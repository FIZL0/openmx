"""R35: what the original's score card did over a partial-backflip capture (r35.toml).

  python3 -I research/r35/summary.py context/capture/r35/fNNN.csv

Prints every row where the flip watch, the wreck, the airborne word, the card's flags, count, combo
or total, or the head node's id changes, and at the end the highest watch stage reached in the air.
"""
import csv
import sys

COLS = ['frame_counter', 'airborne', 'flip_on', 'flip_stage', 'body_up_y', 'body_fwd_y', 'wreck_reason',
        'card_flags', 'card_count', 'card_combo', 'card_total', 'node_id', 'node_points']
KEYS = ['airborne', 'flip_on', 'flip_stage', 'wreck_reason', 'card_flags', 'card_count', 'card_combo',
        'card_total', 'node_id']
rows = list(csv.DictReader(open(sys.argv[1])))
print(len(rows), 'rows', rows[0]['frame_counter'], '..', rows[-1]['frame_counter'])
print(' '.join(COLS))
last = None
for r in rows:
    k = tuple(r.get(c) for c in KEYS)
    if k != last:
        print(' '.join(str(r.get(c)) for c in COLS))
        last = k
print('highest stage', max(int(r['flip_stage'] or 0) for r in rows))
