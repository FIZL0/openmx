"""MD166: print some columns of a capture's rows.  python3 -I research/md166/rows.py CSV FROM TO COL..."""
import csv
import sys
csv.field_size_limit(1 << 24)
rows = list(csv.DictReader(open(sys.argv[1])))
for i in range(int(sys.argv[2]), int(sys.argv[3])):
    print(i, ' | '.join(rows[i][c][:60] for c in sys.argv[4:]))
