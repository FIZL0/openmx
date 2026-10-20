"""P29: the original's attitude through each airborne run of a C6-style capture.

usage: python3 -B research/p29_air_attitude.py NAME [MIN_TICKS]

NAME is a capture in context/capture/acceptance/ without its `.csv` (`recording2-freeride`).

For every run of `on_ground == 0` at least MIN_TICKS long (default 20), prints the chassis' yaw,
pitch and roll at the last ground tick, at takeoff and at the landing, the most roll and the yaw
walked over the run, the largest roll and yaw rates, and how far the left stick was off centre and
whether square (the brake) was held. Angles from `bike_xform`'s rows (0 right, 1 up, 2 forward), in
degrees; rates in degrees a second over the row's own `dt`. Speeds are the body's: the column over
0.8 (P28). Reads the capture only; writes nothing.
"""

import csv
import math
import sys


def angles(r):
    m = [[float(r[f"bike_xform.m{i}{j}"]) for j in range(3)] for i in range(3)]
    right, up, fwd = m
    yaw = math.degrees(math.atan2(fwd[0], fwd[2]))
    pitch = math.degrees(math.asin(max(-1.0, min(1.0, fwd[1]))))
    # roll: the right axis' rise, against the up axis' own height, so a level bike reads 0
    roll = math.degrees(math.atan2(right[1], up[1]))
    return yaw, pitch, roll


def wrap(a):
    while a > 180:
        a -= 360
    while a < -180:
        a += 360
    return a


def stick(r):
    pad = bytes.fromhex(r["pad_a"])
    buttons = ~int.from_bytes(pad[2:4], "little") & 0xFFFF
    lx, ly = pad[6], pad[7]
    return lx - 0x7F, ly - 0x7F, buttons


def main():
    at = f"context/capture/acceptance/{sys.argv[1]}.csv"
    rows = list(csv.DictReader(open(at)))
    least = int(sys.argv[2]) if len(sys.argv) > 2 else 20
    runs, start = [], None
    for i, r in enumerate(rows):
        air = r["on_ground"] == "0"
        if air and start is None:
            start = i
        if not air and start is not None:
            if i - start >= least:
                runs.append((start, i))
            start = None
    for a, b in runs:
        lip, land = rows[a - 1], rows[b]
        tick = lambda r: int(r["frame_counter"])
        speed = math.hypot(*(float(lip[f"bike_vel.{c}"]) for c in "xyz")) / 0.8
        y0, p0, r0 = angles(lip)
        y1, p1, r1 = angles(land)
        seen = [angles(rows[k]) for k in range(a - 1, b + 1)]
        dts = [float(rows[k]["dt"]) for k in range(a, b + 1)]
        roll_most = max(abs(s[2]) for s in seen)
        yaw_rate = max(abs(wrap(seen[k + 1][0] - seen[k][0])) / dts[k] for k in range(len(dts)))
        roll_rate = max(abs(wrap(seen[k + 1][2] - seen[k][2])) / dts[k] for k in range(len(dts)))
        sticks = [stick(rows[k]) for k in range(a, b)]
        sx = max(abs(s[0]) for s in sticks)
        sy = max(abs(s[1]) for s in sticks)
        held = 0
        for s in sticks:
            held |= s[2]
        flip = any(rows[k]["flip_on"] != "0" for k in range(a, b))
        wreck = any(rows[k]["wreck_reason"] != "0" for k in range(a, b + 1))
        print(
            f"{tick(rows[a])}-{tick(rows[b]) - 1} ({b - a:3}) {speed:5.1f} ft/s  "
            f"lip y/p/r {y0:7.1f} {p0:6.1f} {r0:6.1f}  land {wrap(y1 - y0):+6.1f} {p1:6.1f} {r1:6.1f}  "
            f"|roll|max {roll_most:5.1f}  rates yaw {yaw_rate:5.1f} roll {roll_rate:5.1f}  "
            f"stick |x| {sx:3} |y| {sy:3} buttons {held:04x}{'  FLIP' if flip else ''}{'  WRECK' if wreck else ''}"
        )


if __name__ == "__main__":
    main()
