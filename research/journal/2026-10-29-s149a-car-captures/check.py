"""S149a: check a car capture and print what the journal reads off it.

usage: check.py NAME      (context/capture/s149a/NAME.csv)

Frame-counter steps (all 1 for a clean capture), the car's kind, the ticks with all four tires off the
ground (tire +0xb4), each gear with its ratio (engine +0xcc, +0xe4), and how often the slip identities
hold on grounded tires: +0x20c = (v_long - spin * radius) / |v_long|, +0x210 = atan2(v_lat, |v_long|),
with v at +0x228/+0x220, spin +0x1a4 and radius +0x1f8 (|v_long| > 1 ft/s, within 2e-3).
"""
import collections, csv, glob, math, struct, sys

rows = list(csv.DictReader(open(glob.glob('context/capture/s149a/' + sys.argv[1] + '.cs?')[0])))
f = lambda b, o: struct.unpack_from('<f', b, o)[0]
u = lambda b, o: struct.unpack_from('<I', b, o)[0]
TIRES = ('fl', 'fr', 'rl', 'rr')
fc = [int(r['frame_counter']) for r in rows]
print(len(rows), 'rows, frame_counter', fc[0], '..', fc[-1], 'steps', dict(collections.Counter(b - a for a, b in zip(fc, fc[1:]))))
print('kind', sorted(set(r['car_kind'] for r in rows)))
off, n, ratio_ok, angle_ok, gears = [], 0, 0, 0, set()
for i, r in enumerate(rows):
    gears.add((u(bytes.fromhex(r['engine_raw']), 0xcc), round(f(bytes.fromhex(r['engine_raw']), 0xe4), 3)))
    grounded = 0
    for t in TIRES:
        b = bytes.fromhex(r['tire_%s_raw' % t])
        if u(b, 0xb4) != 1:
            continue
        grounded += 1
        vlo, vla, spin, rad = f(b, 0x228), f(b, 0x220), f(b, 0x1a4), f(b, 0x1f8)
        if abs(vlo) > 1:
            n += 1
            ratio_ok += abs((vlo - spin * rad) / abs(vlo) - f(b, 0x20c)) < 2e-3
            angle_ok += abs(math.atan2(vla, abs(vlo)) - f(b, 0x210)) < 2e-3
    if not grounded:
        off.append(i)
runs = []
for i in off:
    if runs and runs[-1][1] == i - 1:
        runs[-1][1] = i
    else:
        runs.append([i, i])
print('all four off:', len(off), 'ticks; runs of 3+ (row, length):', [(a, b - a + 1) for a, b in runs if b - a >= 2])
print('gears', sorted(gears))
print('slip ratio within 2e-3: %d of %d, slip angle: %d' % (ratio_ok, n, angle_ok))
