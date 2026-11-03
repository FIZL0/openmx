"""MD82c: what each flier camera capture holds.

  python3 -I research/md82c/summary.py [NAME ...]

Reads context/capture/md82c/NAME.csv (fly.sh; default all four) and prints, per capture: rows and the
vsync span, dropped ticks, rows with no target block (a wreck), held rows (the camera word for word the
last row's while the target moved), the preset, the near clip and still bit, airborne rows, and the
ranges of speed, yaw and distance, with how many rows sit inside MD82b's fades (|yaw| under pi/2: the
rear fade; speed under 29.3332: the lag-tau scale).
"""
import csv
import math
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
CAMERA = ['cam_dyp', 'cam_lift', 'look0', 'look1', 'look2', 'pos0', 'pos1', 'pos2', 'fovlag', 'vp_eye']
csv.field_size_limit(1 << 24)


def cols(head, name):
    return [i for i, c in enumerate(head) if c == name or c.startswith(name + '.')]


for name in sys.argv[1:] or ['biplane', 'biplane3', 'kiowa', 'kiowa3']:
    with open(os.path.join(ROOT, 'context', 'capture', 'md82c', name + '.csv')) as f:
        r = csv.reader(f)
        head = next(r)
        rows = list(r)
    at = {c: i for i, c in enumerate(head)}
    v = [int(x[at['vsync']]) for x in rows]
    drops = sum(1 for a, b in zip(v, v[1:]) if b != a + 2)
    actor = [x[at['a_flag.x']] != '' for x in rows]
    cam = [i for c in CAMERA for i in cols(head, c)]
    pt = cols(head, 'a_point')
    held = [k for k in range(1, len(rows)) if actor[k] and all(rows[k][i] == rows[k - 1][i] for i in cam)
            and any(rows[k][i] != rows[k - 1][i] for i in pt)]
    live = [x for x, a in zip(rows, actor) if a]
    num = lambda x, c: float(x[at[c]])
    air = sum(1 for x in live if float(x[at['a_flag.x']]) != 0.0)
    speed = [num(x, 'a_flag.y') for x in live]
    yaw = [num(x, 'cam_dyp.y') for x in live]
    dist = [num(x, 'cam_dyp.x') for x in live]
    print(f'{name}: {len(rows)} rows, vsync {v[0]}..{v[-1]}, {drops} dropped, {len(rows) - len(live)} without a target, '
          f'held {held}')
    print(f'  preset {sorted({x[at["cam_preset"]] for x in rows})}, near {sorted({x[at["vp_near"]] for x in live})}, '
          f'still {sorted({int(x[at["still"]]) & 0x800 for x in rows})}, snap {sorted({x[at["cam_snap"]] for x in rows})}, '
          f'lock {sorted({x[at["cam_lock"]] for x in rows})}')
    print(f'  airborne {air}/{len(live)}; speed {min(speed):.2f}..{max(speed):.2f} ({sum(s < 29.3332 for s in speed)} under 29.33, '
          f'{sum(s < 0.0001 for s in speed)} under 0.0001); yaw {min(yaw):.3f}..{max(yaw):.3f} '
          f'({sum(abs(y) < math.pi / 2 for y in yaw)} under pi/2); distance {min(dist):.2f}..{max(dist):.2f}')
