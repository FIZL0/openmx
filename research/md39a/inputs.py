"""MD39a: the car object's f32 words that look like inputs in a capture.

  python3 -I research/md39a/inputs.py NAME

NAME is context/capture/md39a/NAME.csv. Prints each word of obj0/obj1 (+0x0..+0x1800) that stays in
[-1, 1], takes fewer than 12 values and changes 2..60 times, with the rows it changes at; set those
against drive.pad's ticks (row = tick + 1 or 2) to tell which is which.
"""
import csv
import os
import struct
import sys

csv.field_size_limit(1 << 26)
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
rows = list(csv.DictReader(open(os.path.join(ROOT, 'context', 'capture', 'md39a', sys.argv[1] + '.csv'))))
blob = [bytes.fromhex(r['obj0']) + bytes.fromhex(r['obj1']) for r in rows]
for off in range(0, 0x1800, 4):
    v = [struct.unpack_from('<f', b, off)[0] for b in blob]
    ch = [i for i in range(1, len(v)) if v[i] != v[i - 1]]
    if 2 <= len(ch) <= 60 and all(abs(x) <= 1.0001 for x in v) and len(set(v)) < 12:
        print(f'+{off:#06x} {len(ch):3d} changes at {ch[:14]} values {sorted(set(round(x, 3) for x in v))[:8]}')
