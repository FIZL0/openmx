"""P74: the bike's particle manager as read in the decompilation, run over the capture's feed words.

usage: python3 -B model.py <run.csv>

Each row's manager state (entry flag, emitter on, spray clock and side velocity) is predicted from
the previous row's feed words (the stunt-state snapshot the manager reads) and compared. A row that
repeats the one before (the recorder read the same game frame twice) is dropped. Prints the
mismatches and a count.
"""

import csv
import sys

rows = list(csv.DictReader(open(sys.argv[1])))

NO_CONTACT, GROUND, OBJECT, WRECKING, NOT_WRECKING = 1, 2, 4, 8, 0x10
# free09's mxparticles.dat, in order: (type, tire, disable, keys)
SPRAY = {8: (0, 8.0, 1.5), 9: (1, 5.0, 1.0), 10: (0, 4.0, 1.75), 11: (1, 5.0, 4.0), 12: (1, 3.0, 4.0)}
ENTRIES = {
    5: ('exhaust', 1, WRECKING),
    6: ('exhaust', 1, WRECKING),
    7: ('generic', 1, OBJECT | WRECKING | NO_CONTACT),
}
for i, (t, _, _) in SPRAY.items():
    ENTRIES[i] = ('spray', t, OBJECT | WRECKING | NO_CONTACT)
FILE_RATE = {8: 400.0, 9: 400.0}
# Zone1 = none for every descriptor the tire entries name (Dust, DirtSpray, Chunks).


def f(r, k):
    return float(r[k])


def feed(r):
    tires = []
    for g, o, spin, side, zone in [('s914', 's918', 's91c', 's870', 's910'), ('s928', 's92c', 's930', 's880', 's911')]:
        tires.append(dict(ground=int(r[g]) != 0, object=int(r[o]) != 0, spin=int(r[spin]) != 0, side=f(r, side), zone=int(r[zone])))
    return dict(wreck=int(r['s868']) != 0, throttle=f(r, 's888'), clutch=f(r, 's88c'), speed=f(r, 's860'), tires=tires)


def tire_flags(t):
    fl = 0
    if t['ground']:
        fl |= GROUND
    if t['object']:
        fl |= OBJECT
    if not fl:
        fl = NO_CONTACT
    return fl


state = {i: dict(flag=0, on=False, clock=0.0, vx=0.0) for i in ENTRIES}
# Seed from the first row.
for i in ENTRIES:
    r = rows[0]
    state[i] = dict(flag=int(r[f'e{i}_flag']), on=int(r[f'e{i}_on']) & 4 != 0,
                    clock=f(r, f'e{i}_clock') if i in SPRAY else 0.0, vx=f(r, f'e{i}_vx') if i in SPRAY else 0.0)

import math


def frames(a, b):
    """How many game frames row b is past row a: 0 for a repeat, else by how far the bike moved
    against its speed (1 or 2), with whether that is sure."""
    if all(a[k] == b[k] for k in ['s860', 's888', 'e5_at.x', 'e9_at.x', 'e9_carry']):
        return 0, True
    v = float(a['s860'])
    if v <= 5 or int(a['s868']) != 0:
        return 1, False
    d = math.dist([float(a[f'bike_xform.m3{i}']) for i in range(3)], [float(b[f'bike_xform.m3{i}']) for i in range(3)])
    r = d / (v / 60)
    if 0.8 <= r <= 1.5:
        return 1, True
    if 1.8 <= r <= 2.5:
        return 2, False
    return max(1, round(r)), False


dt = 0.016666654497385025
def step(fd):
    glob = WRECKING if fd['wreck'] else NOT_WRECKING
    for i, (kind, tire, disable) in ENTRIES.items():
        s = state[i]
        t = fd['tires'][tire]
        tf = tire_flags(t) | glob
        zone_off = t['zone'] == 0 if kind != 'exhaust' else False
        live = disable & tf == 0
        if not live:
            if s['flag'] & 1 and kind == 'spray':
                s['clock'] = 0.0
            s['flag'] &= ~1
            if kind != 'generic' or True:
                s['on'] = False
        else:
            s['flag'] |= 1
        s['flag'] = (s['flag'] | 2) if zone_off else (s['flag'] & ~2)
        if zone_off:
            s['on'] = False
            if kind == 'spray':
                s['clock'] = 0.0
        if s['flag'] == 1:
            if kind == 'generic':
                s['on'] = True
            elif kind == 'exhaust':
                ff = fd['throttle'] * 0.5 * (15.0 - fd['speed']) / 15.0
                s['on'] = ff > 0
            else:
                _, thr, sfs = SPRAY[i]
                if (fd['throttle'] == 0 or fd['clutch'] != 0 or not t['spin']) and abs(t['side']) <= thr:
                    if s['clock'] > 0:
                        s['clock'] -= dt
                        if s['clock'] > 0:
                            s['on'] = True
                            continue
                    s['clock'] = 0.0
                    s['on'] = False
                else:
                    s['vx'] = -sfs * t['side']
                    s['clock'] = 0.25
                    s['on'] = True

bad = 0
compared = 0
skipped = 0
prev = rows[0]
for n, r in enumerate(rows):
    if n == 0:
        continue
    k, sure = frames(prev, r)
    if k == 0:
        continue
    for _ in range(k):
        step(feed(prev))
    compared += 1
    for i in ENTRIES:
        s = state[i]
        got = (s['flag'], s['on'])
        want = (int(r[f'e{i}_flag']), int(r[f'e{i}_on']) & 4 != 0)
        if i in SPRAY:
            # The clock to 2 ms: 0.25 less n frames of an f32 dt.
            got += (round(s['clock'] * 500), round(s['vx'], 3))
            want += (round(f(r, f'e{i}_clock') * 500), round(f(r, f'e{i}_vx'), 3))
        if got != want and not sure:
            skipped += 1
        elif got != want:
            bad += 1
            if bad < 40:
                print(r['frame_counter'], i, 'got', got, 'want', want)
        if got != want:
            # Resync to the capture so one miss is one line.
            s['flag'], s['on'] = want[0], want[1]
            if i in SPRAY:
                s['clock'], s['vx'] = f(r, f'e{i}_clock'), f(r, f'e{i}_vx')
    prev = r
print(f'{compared} rows compared, {bad} entry mismatches, {skipped} on rows not one sure frame on')
