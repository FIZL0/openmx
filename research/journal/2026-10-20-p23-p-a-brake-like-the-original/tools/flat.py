"""Find fast, flat, straight stretches in a capture: start rows where the next W ticks stay level."""
import csv, glob, math, sys
pat, W = sys.argv[1], int(sys.argv[2])
f = [p for p in glob.glob('context/capture/acceptance/*.csv') if pat in p][0]
r = list(csv.DictReader(open(f)))
P = [(float(x['bike_xform.m30']), float(x['bike_xform.m31']), float(x['bike_xform.m32'])) for x in r]
V = [(float(x['bike_vel.x']), float(x['bike_vel.y']), float(x['bike_vel.z'])) for x in r]
best = []
for i in range(len(r) - W):
    sp = math.hypot(V[i][0], V[i][2])
    if sp < 40:
        continue
    ys = [P[j][1] for j in range(i, i + W)]
    h0 = math.atan2(V[i][0], V[i][2])
    # straightness: max lateral deviation of the path from the start heading
    dev = 0
    for j in range(i, i + W):
        dx, dz = P[j][0] - P[i][0], P[j][2] - P[i][2]
        dev = max(dev, abs(dx * math.cos(h0) - dz * math.sin(h0)))
    yr = max(ys) - min(ys)
    if r[i]['wreck_reason'] != '0':
        continue
    best.append((yr + dev / 4, i, round(sp, 1), round(yr, 1), round(dev, 1), round(math.degrees(h0)), P[i]))
best.sort()
seen = []
for b in best:
    if all(abs(b[1] - s) > W for s in seen):
        seen.append(b[1])
        print(b)
    if len(seen) >= 10:
        break
