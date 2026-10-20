"""Read a brake capture: continuity, then speed / decel per tick from the first square-held row."""
import csv, glob, math, sys
pat = sys.argv[1]
f = [p for p in glob.glob('context/capture/**/*.csv', recursive=True) if pat in p][0]
r = list(csv.DictReader(open(f)))
fc = [int(x['frame_counter']) for x in r]
gaps = [(i, fc[i] - fc[i - 1]) for i in range(1, len(fc)) if fc[i] - fc[i - 1] != 1]
dups = sum(1 for i in range(1, len(r)) if r[i]['bike_xform.m30'] == r[i - 1]['bike_xform.m30'] and r[i]['bike_xform.m32'] == r[i - 1]['bike_xform.m32'])
print(f, len(r), 'rows; fc', fc[0], '..', fc[-1], 'gaps', gaps[:5], 'repeated positions', dups)
print('dt', set(x['dt'] for x in r), 'types', set((x['bike_type'], x['bike_ctl_src']) for x in r))
lo = int(sys.argv[2]) if len(sys.argv) > 2 else 0
hi = int(sys.argv[3]) if len(sys.argv) > 3 else len(r)
step = int(sys.argv[4]) if len(sys.argv) > 4 else 1
prev = None
for i in range(lo, min(hi, len(r)), step):
    x = r[i]
    v = [float(x['bike_vel.' + c]) for c in 'xyz']
    sp = math.hypot(v[0], v[2])
    pa = x['pad_a'][4:20]
    held = ''.join('1' if x[f'ctl{k}.held'] not in ('0', '') else '.' for k in range(16))
    fwd = (float(x['bike_xform.m20']), float(x['bike_xform.m21']), float(x['bike_xform.m22']))
    pitch = math.degrees(math.asin(max(-1, min(1, fwd[1]))))
    print(i, fc[i], 'y', round(float(x['bike_xform.m31']), 2), 'sp', round(sp, 2), 'vy', round(v[1], 2),
          'pitch', round(pitch, 1), pa, held, x['wreck_reason'])
