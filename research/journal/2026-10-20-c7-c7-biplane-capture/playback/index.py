"""Index of a biplane capture: on the ground / in the air, every crash, each control's held runs, and
the sign of each command checked against the rate it makes.

usage: python3 index.py CAPTURE.csv
"""
import csv, sys, math

rows = list(csv.DictReader(open(sys.argv[1])))
F = lambda r, c: float(r[c])
fc0 = int(rows[0]['frame_counter'])
t = lambda k: f"row {k} (tick {fc0 + k}, {k / 30:.1f} s)"

def runs(pred):
    out, s = [], None
    for k, r in enumerate(rows + [None]):
        on = r is not None and pred(r)
        if on and s is None: s = k
        if not on and s is not None: out.append((s, k - 1)); s = None
    return out

print('== on the ground / crash')
for c, p in (('on_ground', lambda r: r['on_ground'] == '1'), ('crash 1', lambda r: r['crash'] == '1'),
             ('crash 13 (thrown)', lambda r: r['crash'] == '13')):
    for a, b in runs(p):
        r = rows[a]; q = rows[a - 1] if a else r
        print(f"{c:18} {t(a)} .. {t(b)}: speed in {F(q, 'speed'):.1f}, y {F(r, 'body_pos.y'):.1f},"
              f" pitch {F(r, 'plane_euler.x'):+.2f}, roll {F(r, 'plane_euler.z'):+.2f}")

print('== controls held (the command values the game wrote)')
for c in ('thr_up', 'thr_down', 'elev_cmd', 'ail_cmd', 'rud_cmd'):
    for sgn in (1, -1):
        rs = runs(lambda r: F(r, c) * sgn > 0.05)
        n = sum(b - a + 1 for a, b in rs)
        if n: print(f"{c:8} {'+' if sgn > 0 else '-'}: {n:4} rows in {len(rs):3} runs, first {t(rs[0][0])}")

print('== throttle')
last = None
for k, r in enumerate(rows):
    v = round(F(r, 'throttle'), 2)
    if v in (0.0, 1.0, round(F(rows[-1], 'throttle'), 2)) or k == 0:
        if v != last: print(f"  {t(k)}: throttle {v}, prop rpm {F(r, 'prop_rpm'):.0f}")
        last = v

print('== signs: command vs the local rate and the euler change over the next tick (correlation)')
def corr(a, b):
    n = len(a); ma = sum(a) / n; mb = sum(b) / n
    sa = math.sqrt(sum((x - ma) ** 2 for x in a)); sb = math.sqrt(sum((y - mb) ** 2 for y in b))
    return sum((x - ma) * (y - mb) for x, y in zip(a, b)) / (sa * sb)
air = [k for k in range(len(rows) - 1) if rows[k]['on_ground'] == '0' and rows[k]['crash'] == '0']
def d(k, c):  # wrapped change of an euler angle from row k to k + 1
    v = F(rows[k + 1], c) - F(rows[k], c)
    return (v + math.pi) % (2 * math.pi) - math.pi
for c in ('elev_cmd', 'pitch_cmd', 'elevator', 'ail_cmd', 'aileron', 'rud_cmd', 'rudder'):
    print(f"  {c:9}", '  '.join(f"rate.{x} {corr([F(rows[k], c) for k in air], [F(rows[k + 1], 'local_rate.' + x) for k in air]):+.2f}" for x in 'xyz'),
          ' ', '  '.join(f"d euler.{x} {corr([F(rows[k], c) for k in air], [d(k, 'plane_euler.' + x) for k in air]):+.2f}" for x in 'xyz'))

print('== extremes in the air')
for c in ('plane_euler.x', 'plane_euler.z', 'body_vel.y', 'body_pos.y', 'speed'):
    lo = min(air, key=lambda k: F(rows[k], c)); hi = max(air, key=lambda k: F(rows[k], c))
    print(f"  {c:14} min {F(rows[lo], c):8.2f} at {t(lo)}   max {F(rows[hi], c):8.2f} at {t(hi)}")

print('== each crash: when it lands, how long it lasts, where the plane comes back')
pos = lambda r: '(' + ', '.join(f"{F(r, 'body_pos.' + c):.1f}" for c in 'xyz') + ')'
print(f"  start position {pos(rows[0])}")
for a, b in runs(lambda r: r['crash'] != '0'):
    down = next((k for k in range(a, b + 1) if rows[k]['crash_down'] != '0'), None)
    print(f"  crash {rows[a]['crash']} {t(a)}: speed {F(rows[a - 1], 'speed'):.1f} -> {F(rows[a], 'speed'):.1f},"
          f" on the ground {rows[a]['on_ground']}, throttle {F(rows[a - 1], 'throttle'):.2f} -> {F(rows[a], 'throttle'):.2f}")
    if down is not None: print(f"    down from {t(down)}, crash_time then {F(rows[down], 'crash_time'):.3f}")
    print(f"    last crash row {t(b)}: crash_time {F(rows[b], 'crash_time'):.3f}, at {pos(rows[b])}")
    if b + 1 < len(rows): print(f"    next row: crash {rows[b + 1]['crash']}, at {pos(rows[b + 1])}, speed {F(rows[b + 1], 'speed'):.1f}")

print('== the pad table the plane reads (pctl[i].value) and the held array (ctl[i]) around each crash end')
cols = [c for c in rows[0] if c.startswith('pctl') or c.startswith('ctl')]
for a, b in runs(lambda r: r['crash'] != '0'):
    for k in range(max(a, b - 3), min(len(rows), b + 2)):
        print(f"  row {k}", ' '.join(f"{c}={rows[k][c]}" for c in cols if rows[k][c] not in ('0', '0.0', '-0.0')))
for a, b in runs(lambda r: r['crash'] != '0'):
    held = [k for k in range(a, b + 1) if any(rows[k][c] == '1' for c in cols if c.startswith('ctl'))]
    print(f"  crash at row {a}: rows with any control held {held[:1]}..{held[-1:]} ({len(held)} rows),"
          f" crash_time at the first {F(rows[held[0]], 'crash_time') if held else None}")

print('== L1 + R1 together (the reset chord) and +0xe0c is not in the spec: rows holding both')
for a, b in runs(lambda r: r['ctl4.held'] == '1' and r['ctl5.held'] == '1'):
    print(f"  {t(a)} .. {t(b)}: crash {rows[a - 1]['crash']} -> {rows[a]['crash']}, on the ground {rows[a - 1]['on_ground']},"
          f" at {pos(rows[a - 1])} -> {pos(rows[a])}")

print('== phases in the air, at least 1 s long (climb: body_vel.y > 10, dive: < -10; bank: |euler.z| > 0.5,'
      ' negative is the way Left/aileron +1 rolls)')
fly = lambda r: r['on_ground'] == '0' and r['crash'] == '0'
for name, p in (('climb', lambda r: F(r, 'body_vel.y') > 10), ('dive', lambda r: F(r, 'body_vel.y') < -10),
                ('bank left', lambda r: F(r, 'plane_euler.z') < -0.5), ('bank right', lambda r: F(r, 'plane_euler.z') > 0.5)):
    for a, b in runs(lambda r: fly(r) and p(r)):
        if b - a + 1 < 30: continue
        seg = rows[a:b + 1]
        print(f"  {name:10} {t(a)} .. {t(b)}: y {F(seg[0], 'body_pos.y'):.0f} -> {F(seg[-1], 'body_pos.y'):.0f},"
              f" speed {F(seg[0], 'speed'):.0f} -> {F(seg[-1], 'speed'):.0f}, euler.x {min(F(r, 'plane_euler.x') for r in seg):+.2f}..{max(F(r, 'plane_euler.x') for r in seg):+.2f},"
              f" euler.z {min(F(r, 'plane_euler.z') for r in seg):+.2f}..{max(F(r, 'plane_euler.z') for r in seg):+.2f}")

print('== take-offs: the last row on the ground of a run that started still')
for a, b in runs(lambda r: r['on_ground'] == '1'):
    if F(rows[a], 'speed') < 2 and b + 1 < len(rows):
        print(f"  {t(b + 1)}: off the ground at {F(rows[b], 'speed'):.1f}, throttle {F(rows[b], 'throttle'):.2f}, elevator {F(rows[b], 'elev_cmd'):+.2f}")
