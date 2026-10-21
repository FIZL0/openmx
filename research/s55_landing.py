"""S55: how fast the original's attitude moves through a touchdown, from a C6-style capture.

usage: python3 -B research/s55_landing.py NAME [MIN_AIR]

NAME is a capture in context/capture/acceptance/ without its `.csv` (`recording2-tricks`).

For every touchdown after at least MIN_AIR airborne ticks (default 8), skipping put-backs (`fresh`
near it) and wrecks (`wreck_reason` set in the window), prints the yaw/roll/crossup from 3 ticks
before to 8 after, the most the heading and the roll moved in one tick over that window, and the
crossup (heading against the velocity's direction) two ticks after the touchdown. The last lines are
the largest of each over all the landings: `ORIGINAL_LANDING_TURN` and `ORIGINAL_LANDING_CROSSUP`
in crates/game-sim/tests/ride.rs. Angles in degrees, as research/p29_air_attitude.py reads them.
Reads the capture only; writes nothing.
"""

import csv
import math
import sys


def angles(r):
    m = [[float(r[f"bike_xform.m{i}{j}"]) for j in range(3)] for i in range(3)]
    right, up, fwd = m
    yaw = math.degrees(math.atan2(fwd[0], fwd[2]))
    roll = math.degrees(math.atan2(right[1], up[1]))
    velyaw = math.degrees(math.atan2(float(r["bike_vel.x"]), float(r["bike_vel.z"])))
    return yaw, roll, wrap(yaw - velyaw)


def wrap(a):
    return (a + 180.0) % 360.0 - 180.0


def main():
    name = sys.argv[1]
    min_air = int(sys.argv[2]) if len(sys.argv) > 2 else 8
    rows = list(csv.DictReader(open(f"context/capture/acceptance/{name}.csv")))
    ground = [int(r["on_ground"]) for r in rows]
    worst = [0.0, 0.0, 0.0]
    for k in range(3, len(rows) - 9):
        if not ground[k] or ground[k - 1]:
            continue
        s = k - 1
        while s >= 0 and not ground[s]:
            s -= 1
        if k - 1 - s < min_air:
            continue
        window = range(k - 3, k + 9)
        if any(rows[j]["fresh"] == "1" for j in window):
            continue
        if any(rows[j]["wreck_reason"] != "0" for j in window):
            continue
        a = [angles(rows[j]) for j in window]
        turn = max(abs(wrap(y1 - y0)) for (y0, _, _), (y1, _, _) in zip(a, a[1:]))
        roll = max(abs(r1 - r0) for (_, r0, _), (_, r1, _) in zip(a, a[1:]))
        after = a[5][2]
        print(f"landing at row {k} after {k - 1 - s} airborne: most a tick heading {turn:.2f} "
              f"roll {roll:.2f}, crossup {a[2][2]:+.2f} -> {after:+.2f} two ticks after")
        print("  yaw/roll/crossup " + " ".join(f"{y:+.1f}/{r:+.1f}/{c:+.1f}" for y, r, c in a))
        worst = [max(worst[0], turn), max(worst[1], roll), max(worst[2], abs(after))]
    print(f"most over all: heading {worst[0]:.2f}, roll {worst[1]:.2f} a tick; "
          f"crossup two ticks after {worst[2]:.2f}")


main()
