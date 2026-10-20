"""P32: does the original bring the bike back upright and straight after a turn or a slide?

usage: python3 -B research/p32_recover.py NAME [dump]

NAME is a capture in context/capture/acceptance/ without its `.csv` (`recording2-freeride`).

With `dump`, prints one line a row: tick, ground flags, stick x/y, speed, the chassis' roll and its
slip (heading less the direction of travel on the ground plane), in degrees. Without it, finds every
release: a ground tick where the left stick x comes back inside the player's dead zone (±0.3) after
at least 4 ticks outside it, and follows roll and slip from there while the stick stays centred and
the bike stays down, printing when each first falls under 3 deg and stays there 5 ticks.

Angles from `bike_xform`'s rows (0 right, 1 up, 2 forward), as research/p29_air_attitude.py reads
them. Speeds are the body's: the column over 0.8 (P28). Stick from pad_a, the tape's own field
(recording2's rows poll the pad of frame - 2; see the memory note, so the stick leads the chassis
by up to two ticks). The first recording's capture (acceptance-freeride) has no on_ground or
both_wheels columns, so there every row counts as grounded and the jumps have to be read off the
row's height by hand. Reads the capture only; writes nothing.
"""

import csv
import math
import sys


def angles(r):
    m = [[float(r[f"bike_xform.m{i}{j}"]) for j in range(3)] for i in range(3)]
    right, up, fwd = m
    yaw = math.degrees(math.atan2(fwd[0], fwd[2]))
    roll = math.degrees(math.atan2(right[1], up[1]))
    return yaw, roll


def wrap(a):
    while a > 180:
        a -= 360
    while a < -180:
        a += 360
    return a


def stick(r):
    pad = bytes.fromhex(r["pad_a"])
    buttons = ~int.from_bytes(pad[2:4], "little") & 0xFFFF
    return (pad[6] - 0x7F) / 127.0, (pad[7] - 0x7F) / 127.0, buttons


def row(r):
    yaw, roll = angles(r)
    vx, vz = float(r["bike_vel.x"]), float(r["bike_vel.z"])
    speed = math.hypot(vx, float(r["bike_vel.y"]), vz) / 0.8
    flat = math.hypot(vx, vz) / 0.8
    slip = wrap(yaw - math.degrees(math.atan2(vx, vz))) if flat > 2.0 else float("nan")
    sx, sy, held = stick(r)
    return dict(
        tick=int(r["frame_counter"]),
        ground=r.get("on_ground", "1") == "1",
        both=r.get("both_wheels", "1") == "1",
        wreck=r["wreck_reason"] != "0",
        sx=sx,
        sy=sy,
        held=held,
        speed=speed,
        roll=roll,
        slip=slip,
        yaw=yaw,
        dt=float(r["dt"]),
    )


def settled(seq, key, after, limit=3.0, hold=5):
    for k in range(after, len(seq) - hold + 1):
        if all(abs(seq[j][key]) < limit for j in range(k, k + hold)):
            return k
    return None


def main():
    at = f"context/capture/acceptance/{sys.argv[1]}.csv"
    rows = [row(r) for r in csv.DictReader(open(at))]
    if len(sys.argv) > 2 and sys.argv[2] == "dump":
        for s in rows:
            print(
                f"{s['tick']} g{int(s['ground'])}{int(s['both'])} w{int(s['wreck'])} "
                f"sx {s['sx']:+.2f} sy {s['sy']:+.2f} b {s['held']:04x} v {s['speed']:5.1f} "
                f"roll {s['roll']:+6.1f} slip {s['slip']:+6.1f}"
            )
        return
    dead = 0.3
    i = 4
    while i < len(rows):
        s = rows[i]
        turned = all(abs(rows[k]["sx"]) >= dead for k in range(i - 4, i))
        if not (abs(s["sx"]) < dead and turned and s["ground"] and not s["wreck"]):
            i += 1
            continue
        # follow while centred, grounded and not wrecked
        seq = []
        k = i
        while k < len(rows) and abs(rows[k]["sx"]) < dead and rows[k]["ground"] and not rows[k]["wreck"]:
            seq.append(rows[k])
            k += 1
        r_at = settled(seq, "roll", 0)
        s_at = settled([x for x in seq if x["slip"] == x["slip"]], "slip", 0)
        peak_roll = max(abs(x["roll"]) for x in seq)
        print(
            f"release {s['tick']} for {len(seq):3} ticks  v {s['speed']:5.1f}  "
            f"roll {s['roll']:+6.1f} slip {s['slip']:+6.1f}  |roll|max {peak_roll:5.1f}  "
            f"roll<3 at +{r_at}  slip<3 at +{s_at}  both {sum(x['both'] for x in seq)}/{len(seq)}  "
            f"end roll {seq[-1]['roll']:+6.1f} slip {seq[-1]['slip']:+6.1f} v {seq[-1]['speed']:5.1f}"
        )
        i = k


if __name__ == "__main__":
    main()
