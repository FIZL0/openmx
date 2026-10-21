#!/usr/bin/env python3
"""S74: find the human's rider body actor from a loaded state and print its skeleton's layout.

    ./kit pcsx2 with -- python3 -B research/journal/2026-10-21-s74-putback-bones/capture/probe_body.py --state 6

Research, not product.
"""

import argparse
import os
import struct
import sys
import time

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..', '..', '..'))
sys.path.insert(0, ROOT)
sys.path.insert(0, os.path.dirname(__file__))

from ps2kit.pine import Pine  # noqa: E402
from bones_common import human_bike, rider_body  # noqa: E402


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument('--state', type=int)
    a = ap.parse_args()
    p = Pine()
    if a.state is not None:
        p.load_state(a.state)
        t0 = time.time()
        while time.time() - t0 < 2.5:
            time.sleep(0.05)
    bike = human_bike(p)
    body = rider_body(p, bike)
    print(f'bike {bike:08x} body actor {body:08x}')
    if not body:
        return 1
    n = p.read32(body + 0x230)
    arr = p.read32(body + 0x234)
    print(f'joints {n} at {arr:08x}, dir {p.read_block(body + 0x1e0, 16)!r}')
    print(f'mode +0x47c {p.read32(body + 0x47c)} prev {p.read32(body + 0x480)} req {p.read32(body + 0x488):08x}'
          f' parts +0x2e0 {p.read32(body + 0x2e0)} +0x2f4 {p.read32(body + 0x2f4):08x}')
    bnode = p.read32(bike + 0x18)
    print(f'bike node {bnode:08x} w3 {struct.unpack("<4f", p.read_block(bnode + 0xb0, 16))}')
    for j in range(n):
        rec = arr + 0x60 * j
        node = p.read32(rec + 0x34)
        b = p.read_block(node, 0xc0)
        name_ptr = p.read32(node + 0x4)
        parent = p.read32(node + 0x8)
        w3 = struct.unpack_from('<4f', b, 0xb0)
        l3 = struct.unpack_from('<4f', b, 0x70)
        print(f'{j:2} rec {rec:08x} node {node:08x} parent? {parent:08x} words0-3 '
              f'{" ".join(f"{x:08x}" for x in struct.unpack_from("<4I", b, 0))} l3 {l3} w3 {w3}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
