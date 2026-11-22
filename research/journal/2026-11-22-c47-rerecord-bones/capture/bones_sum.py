#!/usr/bin/env python3
"""C47: what a bones capture (S74's `bones_watch.py`, S201's `entry_watch.py`) holds, in one line each.

    bones_sum.py context/s74 [context/s201 ...]

Prints the frames, torn/gaps, the put-backs (the body going to mode 0: chord when the ghost clock starts
below 3.0, clock when it starts at 3.0, as rider_anim.rs tells them) and the wreck entries (the bike's
reason going from 0 to not 0). Research, not product.
"""

import glob
import os
import struct
import sys


def rows(path):
    out, row = [], None
    tail = ''
    for line in open(path):
        if line.startswith('#'):
            tail = line.strip()
            continue
        k, _, v = line.rstrip('\n').partition(' ')
        if k == 'f':
            row = {'f': int(v)}
            out.append(row)
        elif row is not None and k in ('body.47c', 'bike.748', 'bike.7dc'):
            row[k] = bytes.fromhex(v)
    return out, tail


def main():
    # A folder stands for every .txt in it (the shell's guard refuses big capture files named outright).
    paths = []
    for arg in sys.argv[1:]:
        paths += sorted(glob.glob(os.path.join(arg, '*.txt'))) if os.path.isdir(arg) else [arg]
    for path in paths:
        rs, tail = rows(path)
        chords = clocks = wrecks = 0
        events = []
        for a, b in zip(rs, rs[1:]):
            mode_a = struct.unpack_from('<I', a['body.47c'])[0]
            mode_b = struct.unpack_from('<I', b['body.47c'])[0]
            if mode_b == 0 and mode_a != 0:
                ghost = struct.unpack_from('<f', b['bike.7dc'])[0]
                kind = 'clock' if ghost == 3.0 else 'chord'
                chords += kind == 'chord'
                clocks += kind == 'clock'
                events.append(f'{kind}@{b["f"]}')
            if struct.unpack_from('<I', a['bike.748'])[0] == 0 and struct.unpack_from('<I', b['bike.748'])[0] != 0:
                wrecks += 1
                events.append(f'wreck@{b["f"]}')
        print(f'{path}: {len(rs)} rows {rs[0]["f"]}..{rs[-1]["f"]} {tail}; '
              f'{chords} chord + {clocks} clock put-backs, {wrecks} wreck entries: {" ".join(events)}')


if __name__ == '__main__':
    main()
