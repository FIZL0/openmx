"""MD166: a capture's row count and its vsync steps that are not +2.  python3 -I research/md166/gaps.py CSV"""
import csv
import sys
csv.field_size_limit(1 << 24)
rows = list(csv.DictReader(open(sys.argv[1])))
v = [int(x['vsync']) for x in rows]
gaps = [(i, v[i] - v[i - 1]) for i in range(1, len(v)) if v[i] - v[i - 1] != 2]
print(len(rows), 'rows', v[0], '..', v[-1], len(gaps), 'gaps', gaps[:60])
