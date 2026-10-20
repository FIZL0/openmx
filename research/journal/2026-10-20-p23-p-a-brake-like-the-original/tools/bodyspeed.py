import csv, glob, math, sys
f = [p for p in glob.glob('context/capture/**/*.csv', recursive=True) if sys.argv[1] in p][0]
r = list(csv.DictReader(open(f)))
dt = float(r[0]['dt'])
def sp(i):
    a, b = r[i - 1], r[i]
    return math.hypot(float(b['bike_xform.m30']) - float(a['bike_xform.m30']), float(b['bike_xform.m32']) - float(a['bike_xform.m32'])) / dt
press = next(i for i in range(1, len(r)) if r[i]['pad_a'][4:8] == 'ff7f')
s = {i: sp(i) for i in range(1, len(r))}
print('square from row', press, 'dt', dt)
for i in range(press - 1, press + 6):
    print(i, round(s[i], 2))
def cross(v, start):
    return next(i for i in range(start, len(r)) if s[i] < v)
start = press + 1  # the last row still gaining before the lever bites
for i in range(press, press + 4):
    if s[i + 1] < s[i]:
        start = i; break
a, b = cross(60.0, start), cross(12.0, start)
print('start row', start, 'speed', round(s[start], 2))
print('60 -> row', a, round(s[a], 2), '12 -> row', b, round(s[b], 2), 'decel', round((s[a] - s[b]) / ((b - a) * dt), 2))
st = cross(1.0, start)
print('under 1 at row', st, round(s[st], 3), 'time', round((st - start) * dt, 3))
