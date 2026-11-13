"""C33: merge captures of the same ride from the same state into one csv of the rows they agree on.

A PINE read can land while the game is mid-tick (one rider's body half-integrated), and where it lands
moves with the emulator's speed. So the same ride recorded at different speeds tears different rows,
and a row two recordings hold byte for byte is the game's own end-of-tick row. For each frame_counter
the output takes the row that most inputs hold, and adds `agree` (how many inputs held it) and `of` (how
many inputs had the frame). A row with agree < 2 is untrusted; a replay should skip comparing it.

usage: agree.py OUT.csv IN.csv IN.csv [IN.csv ...]
"""
import collections, csv, sys

out, ins = sys.argv[1], sys.argv[2:]
runs = []
for path in ins:
    with open(path) as f:
        rows = list(csv.DictReader(f))
    runs.append({int(r['frame_counter']): r for r in rows})
cols = [c for c in runs[0][next(iter(runs[0]))] if c != 'frame']
frames = sorted(set().union(*runs))
n = {k: 0 for k in range(len(ins) + 1)}
with open(out, 'w', newline='') as f:
    w = csv.writer(f)
    w.writerow(cols + ['agree', 'of'])
    for fc in frames:
        have = [tuple(r[fc][c] for c in cols) for r in runs if fc in r]
        row, k = collections.Counter(have).most_common(1)[0]
        n[k] += 1
        w.writerow(list(row) + [k, len(have)])
print('%d frames %d..%d; rows by agreement:' % (len(frames), frames[0], frames[-1]),
      ', '.join('%d: %d' % (k, v) for k, v in n.items() if v))
