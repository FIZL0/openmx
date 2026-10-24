"""S67: the original's bike through each touchdown of S109's animandpump capture, tick by tick.

usage: python3 -B research/s67_landing.py [CSV] [BEFORE] [AFTER]

CSV defaults to context/capture/s109/run1.csv (frame_counter 1939-3142, no wreck, one row a tick). For
each tick the airborne flag drops, prints from BEFORE ticks before (default 4) to AFTER after (default
12): the drawn bike's pitch, roll and yaw, the chassis' angular velocity in its own frame, both
suspensions' compression, the vertical speed, and the airborne/on-ground flags. Angles in degrees, as
research/s55_landing.py reads them. A row whose counter is not one past the last is marked `torn`.
Reads the capture only; writes nothing.
"""

import csv
import math
import sys


def wrap(a):
    return (a + 180.0) % 360.0 - 180.0


def attitude(r):
    m = [[float(r[f"bike_xform.m{i}{j}"]) for j in range(3)] for i in range(3)]
    right, up, fwd = m
    yaw = math.degrees(math.atan2(fwd[0], fwd[2]))
    pitch = math.degrees(math.asin(max(-1.0, min(1.0, fwd[1]))))
    roll = math.degrees(math.atan2(right[1], up[1]))
    return pitch, roll, yaw


def main():
    path = sys.argv[1] if len(sys.argv) > 1 else "context/capture/s109/run1.csv"
    before = int(sys.argv[2]) if len(sys.argv) > 2 else 4
    after = int(sys.argv[3]) if len(sys.argv) > 3 else 12
    rows = list(csv.DictReader(open(path)))
    fc = [int(r["frame_counter"]) for r in rows]
    air = [int(r["airborne"]) for r in rows]
    for k in range(1, len(rows)):
        if not (air[k - 1] and not air[k]):
            continue
        s = k - 1
        while s >= 0 and air[s]:
            s -= 1
        print(f"touchdown at {fc[k]} after {k - 1 - s} airborne ticks")
        print("   tick  pitch   roll    yaw  dyaw | ang_vel x/y/z (own frame) | f_comp r_comp |  vy   | air gnd both")
        last = None
        for j in range(max(0, k - before), min(len(rows), k + after + 1)):
            r = rows[j]
            p, ro, y = attitude(r)
            dy = wrap(y - last) if last is not None else 0.0
            last = y
            w = [float(r[f"ch_ang_vel.{c}"]) for c in "xyz"]
            torn = "" if j == 0 or fc[j] == fc[j - 1] + 1 else " torn"
            print(f"  {fc[j]:5d} {p:+6.1f} {ro:+6.1f} {y:+6.1f} {dy:+5.1f} | {w[0]:+6.2f} {w[1]:+6.2f} {w[2]:+6.2f} |"
                  f" {float(r['f_comp']):.3f}  {float(r['r_comp']):.3f} | {float(r['bike_vel.y']):+6.1f} |"
                  f"  {r['airborne']}   {r['on_ground']}    {r['both_wheels']}{torn}")


main()
