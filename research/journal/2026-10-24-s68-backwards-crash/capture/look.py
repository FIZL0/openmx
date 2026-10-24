"""S68: a short look at one backwards capture, every Nth row (S58's look.py with S68's lanes).

    python3 -B look.py CASE [EVERY] [COLUMN...]

CASE names context/capture/s68-backwards/CASE.csv (or DIR/CASE another folder of context/capture); the default columns are the reverse and the wreck
test's words."""
import csv
import os
import sys

here = os.path.dirname(os.path.abspath(__file__))
root = os.path.abspath(os.path.join(here, '..', '..', '..', '..'))
where = sys.argv[1] if '/' in sys.argv[1] else 's68-backwards/' + sys.argv[1]
path = os.path.join(root, 'context', 'capture', where + '.csv')
every = int(sys.argv[2]) if len(sys.argv) > 2 else 10
rows = list(csv.DictReader(open(path)))
cols = sys.argv[3:] or ['gas_lever', 'brake_lever', 'reverse', 'bike_speed', 'ch_speed', 'stick_x', 'lean_x',
                        'ch_pitch', 'ch_roll', 'on_ground', 'airborne', 'g_peak.x', 'g_peak.y', 'g_peak.z',
                        'wreck_reason']
cols = [c for c in cols if c in rows[0]]
print(len(rows), 'rows;', 'frame_counter', ' '.join(cols))
for i, r in enumerate(rows):
    if i % every == 0 or (i and r.get('wreck_reason') != rows[i - 1].get('wreck_reason')):
        print(r['frame_counter'], ' '.join(r[c][:8] for c in cols))
