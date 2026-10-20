"""I7: which pad buffer each row's player-record stick came from. Reads the capture on stdin.

A row carries two buffers, stamped f and f-1 (f = the row's frame); the row before carries f-2 and f-3.
For each row, every candidate whose left stick (bytes 6, 7) through ±0.3 is within 1e-6 of the row's
stick_x / stick_y is counted. Only rows where the candidates disagree with each other decide anything.
"""
import collections
import csv
import sys


def apply(raw, dead=0.3):
    v = -1.0 + raw / 255.0 * 2.0
    if v < dead:
        return (v + dead) / (1 - dead) if v <= -dead else 0.0
    return (v - dead) / (1 - dead)


rows = list(csv.DictReader(sys.stdin))
by_stamp = {}
for r in rows:
    for h in "ab":
        by_stamp[int(r["stamp_" + h])] = bytes.fromhex(r["pad_" + h])
counts = collections.Counter()
decided = 0
for r in rows[1:]:
    f = int(r["frame"])
    got = (float(r["stick_x"]), float(r["stick_y"]))
    cands = {}
    for back in range(5):
        pad = by_stamp.get(f - back)
        if pad is not None:
            cands[back] = (apply(pad[6]), apply(pad[7]))
    if len(set(cands.values())) < 2:
        continue
    decided += 1
    hit = tuple(b for b, s in cands.items() if all(abs(s[i] - got[i]) < 1e-6 for i in range(2)))
    counts[hit] += 1
print(f"{decided} rows where the buffers disagree; which stamps f-n explain the stick:")
for hit, n in counts.most_common():
    print(f"  f-{hit}: {n}")
