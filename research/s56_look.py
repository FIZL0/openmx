"""S56: one line every N rows of an s56 capture: the bike, the wreck and the main bone.

usage: python3 -B research/s56_look.py NAME [EVERY] [FROM] [TO]

Reads context/capture/s56/NAME.csv; writes nothing.
"""

import csv
import math
import sys


def main():
    name = sys.argv[1]
    every = int(sys.argv[2]) if len(sys.argv) > 2 else 10
    lo = int(sys.argv[3]) if len(sys.argv) > 3 else 0
    hi = int(sys.argv[4]) if len(sys.argv) > 4 else 1 << 30
    rows = list(csv.DictReader(open(f'context/capture/s56/{name}.csv')))
    for k, r in enumerate(rows):
        if k % every or not lo <= k <= hi:
            continue
        fl = lambda c: float(r[c]) if r[c] else math.nan
        v = [fl(f'bike_vel.{a}') for a in 'xyz']
        pos = [fl(f'bike_xform.m3{i}') for i in range(3)]
        b = [fl(f'bone0.pos.{a}') for a in 'xyz']
        bv = [fl(f'bone0.vel.{a}') for a in 'xyz']
        held = ''.join('TOXS'[i] if r[f'ctl{i}.held'] == '1' else '.' for i in range(4))
        touch = ''.join(r[f'col{i}.touched'] or '-' for i in range(9))
        holds = ''.join(r[f'hold{i}_on'] or '-' for i in range(4))
        print(f"{k:4} f{r['frame_counter']} w{r['wreck_reason']} ride{r['a_riding']} mode{r['a_mode']} "
              f"st{r['a_stiff']} off{r['a_off_bike']} set{r['a_settled']} fl{r['a_flung']} h{holds} t{touch} {held} "
              f"bike ({pos[0]:.1f},{pos[1]:.1f},{pos[2]:.1f}) |v| {math.hypot(*v):.1f} "
              f"b0 ({b[0]:.2f},{b[1]:.2f},{b[2]:.2f}) |bv| {math.hypot(*bv):.2f}")


main()
