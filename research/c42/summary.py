"""C42: the rows where a watched word changes (card 0's combo and bank, the AI banks, laps, the manager's
flags, in_game, player 0's points). usage: summary.py CSV"""
import csv, sys

WORDS = ['in_game', 'points', 'mgr_flags', 'mgr_laps', 'c0_laps', 'c0_nodes', 'c0_bank', 'c1_bank', 'c2_bank',
         'c3_bank']
r = csv.DictReader(open(sys.argv[1]))
last, n, prev_f = {}, 0, None
for row in r:
    n += 1
    changed = [w for w in WORDS if row.get(w) != last.get(w)]
    if changed:
        print(row['frame_counter'], ' '.join(f'{w}={row.get(w)}' for w in changed),
              f'(c0 flags {int(row["c0_flags"] or 0):#x} combo {row["c0_combo"]})' if 'c0_nodes' in changed else '')
    last = row
    prev_f = row['frame_counter']
print(n, 'rows, last frame', prev_f)
