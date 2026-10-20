"""I7: which pad buffer each row's control state (ctl*.held) came from, by the number of buttons
down (bytes 2-3, active low) against the number of held controls. Reads the capture on stdin."""
import collections
import csv
import sys

rows = list(csv.DictReader(sys.stdin))
held_cols = [c for c in rows[0] if c.startswith("ctl") and c.endswith(".held")]
by_stamp = {}
for r in rows:
    for h in "ab":
        by_stamp[int(r["stamp_" + h])] = bytes.fromhex(r["pad_" + h])
down = lambda p: bin(~(p[2] | p[3] << 8) & 0xFFFF).count("1")
counts = collections.Counter()
for r in rows[1:]:
    f = int(r["frame"])
    cands = {b: down(by_stamp[f - b]) for b in range(4) if f - b in by_stamp}
    if len(set(cands.values())) < 2:
        continue
    n = sum(int(r[c]) != 0 for c in held_cols)
    counts[tuple(b for b, d in cands.items() if d == n)] += 1
print(f"{len(held_cols)} held columns; which stamps f-n explain the held count:")
for hit, n in counts.most_common():
    print(f"  f-{hit}: {n}")
