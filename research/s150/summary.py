"""S150: a capture's landing, row by row: the heading (from ch_quat), the world yaw rate, the velocity's
own heading, the height and the wreck reason. usage: python3 -B research/s150/summary.py CSV [EVERY]"""
import csv, math, sys
rows = list(csv.DictReader(open(sys.argv[1])))
every = int(sys.argv[2]) if len(sys.argv) > 2 else 1
for i, r in enumerate(rows):
    if i % every:
        continue
    x, y, z, w = (float(r['ch_quat.' + k]) for k in 'xyzw')
    # forward = the rotated +z
    fx, fz = 2 * (x * z + w * y), 1 - 2 * (x * x + y * y)
    ux = 2 * (x * y - w * z); uy = 1 - 2 * (x * x + z * z)
    vx, vy, vz = (float(r['ch_vel.' + k]) for k in 'xyz')
    print('%s h %7.1f vh %7.1f s %5.1f vy %6.1f up %5.2f wy %6.2f y %7.2f reason %s' % (
        r['frame_counter'], math.degrees(math.atan2(fx, fz)), math.degrees(math.atan2(vx, vz)),
        math.hypot(vx, vz), vy, uy, float(r['ch_world_ang_vel.y']), float(r['ch_cofg.y']), r['wreck_reason']))
