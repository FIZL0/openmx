"""The index of recording2's capture: wrecks, tricks, backflips, air, plain riding. tick = frame_counter.

   python3 -I index.py [capture.csv]      # from the repo root; the journal entry quotes its output"""
import csv, math, sys
P = sys.argv[1] if len(sys.argv) > 1 else 'context/capture/acceptance/recording2-freeride.csv'
rows = list(csv.DictReader(open(P)))
CTL = ['tri', 'cir', 'X', 'sq', 'L1', 'R1', 'L2', 'R2', 'sel', 'start', 'R3', 'L3', 'up', 'right', 'down', 'left']
NAMES = ['taunt', 'lazyboy', 'cancan', 'catwalk', 'nohandedlander', 'pendulum', 'nacnac', 'flatliner',
         'hartattack', 'indianair', 'barhop', 'supercan', 'kod', 'superman', 'saranwrap', 'stalelin',
         'tsunami', 'onehandindianair', 'shoalin', 'cordova', 'deadbody', 'rigamortis',
         'supercanindianair', '?23']
SPECIAL = {24: 'BackFlip', 25: 'Wheelie(lean)', 26: 'Stoppie', 28: 'none'}


def name(i):
    return NAMES[i] if i < 24 else SPECIAL.get(i, '?')


T = []
for r in rows:
    a, sa = bytes.fromhex(r['pad_a']), int(r['stamp_a'])
    b, sb = bytes.fromhex(r['pad_b']), int(r['stamp_b'])
    pad = a if sa < sb else b
    d = {k: r[k] for k in r}
    d['t'] = int(r['frame_counter'])
    d['held'] = [i for i in range(16) if int(r[f'ctl{i}.held'])]
    d['lx'], d['ly'], d['rx'], d['ry'] = pad[6], pad[7], pad[4], pad[5]
    vx, vy, vz = float(r['bike_vel.x']), float(r['bike_vel.y']), float(r['bike_vel.z'])
    d['spd'] = math.hypot(vx, vz)
    d['pos'] = (float(r['bike_xform.m30']), float(r['bike_xform.m31']), float(r['bike_xform.m32']))
    for k in ('wreck_reason', 'on_ground', 'airborne', 'flip_on', 'flip_arm', 'flip_torqued', 'flip_ramp',
              'trick_running', 'trick_id', 'trick_req', 'trick_group', 'trick_dir', 'trick_safe',
              'trick_armed', 'trick_count', 'fresh', 'both_wheels'):
        d[k] = int(r[k])
    d['ft'] = float(r['flip_torque.x']), float(r['flip_torque.y']), float(r['flip_torque.z'])
    T.append(d)


def runs(pred):
    out, s = [], None
    for i, d in enumerate(T):
        if pred(d):
            if s is None:
                s = i
        elif s is not None:
            out.append((s, i - 1)); s = None
    if s is not None:
        out.append((s, len(T) - 1))
    return out


def held(i):
    return '+'.join(CTL[c] for c in T[i]['held']) or '-'


print('ticks', T[0]['t'], '..', T[-1]['t'], 'tape tick = tick - 2375')
print('\n== wrecks ==')
for s, e in runs(lambda d: d['wreck_reason'] != 0):
    rs = sorted({T[i]['wreck_reason'] for i in range(s, e + 1)})
    p = T[s - 1]
    print(f"{T[s]['t']}-{T[e]['t']} ({e - s + 1} ticks) reason {rs}; tick before: ground {p['on_ground']} air {p['airborne']} spd {p['spd']:.1f} "
          f"trick {p['trick_running']}/{name(p['trick_id'])} safe {p['trick_safe']} flip {p['flip_on']}/{p['flip_arm']} held {held(s - 1)} "
          f"stick l {p['lx']},{p['ly']}")
    hist = []
    for i in range(max(0, s - 12), s + 2):
        hist.append(f"{T[i]['t']}:{held(i)}/g{T[i]['on_ground']}/r{T[i]['wreck_reason']}")
    print('   ', ' '.join(hist))
    print('    after: fresh ticks', [T[i]['t'] for i in range(e, min(len(T), e + 40)) if T[i]['fresh']])

print('\n== airborne runs (>= 6 ticks) ==')
for s, e in runs(lambda d: d['airborne'] == 1):
    if e - s + 1 >= 6:
        land = T[e + 1] if e + 1 < len(T) else None
        w = next((T[i]['t'] for i in range(e + 1, min(len(T), e + 15)) if T[i]['wreck_reason']), None)
        print(f"{T[s]['t']}-{T[e]['t']} ({e - s + 1}) takeoff spd {T[s - 1]['spd']:.1f} land {land and land['t']} "
              f"land spd {land and round(land['spd'], 1)} wreck-after {w}")

print('\n== tricks (trick_running runs, and requests) ==')
for s, e in runs(lambda d: d['trick_running'] == 1):
    ids = []
    for i in range(s, e + 1):
        if not ids or ids[-1][0] != T[i]['trick_id']:
            ids.append((T[i]['trick_id'], T[i]['t']))
    safe = next((T[i]['t'] for i in range(s, e + 1) if T[i]['trick_safe']), None)
    land = next((T[i]['t'] for i in range(s, min(len(T), e + 30)) if T[i]['on_ground']), None)
    w = next(((T[i]['t'], T[i]['wreck_reason']) for i in range(s, min(len(T), e + 10)) if T[i]['wreck_reason']), None)
    p = T[s - 1]
    print(f"{T[s]['t']}-{T[e]['t']} ({e - s + 1}) ids {[(i, name(i), t) for i, t in ids]} gesture g{T[s]['trick_group']} d{T[s]['trick_dir']} "
          f"(tick before g{p['trick_group']} d{p['trick_dir']} held {held(s - 1)}) safe {safe} first ground {land} wreck {w}")
print('trick_req changes:')
prev = None
for d in T:
    k = (d['trick_req'], d['trick_id'])
    if k != prev:
        print(f"  {d['t']}: req {d['trick_req']} {name(d['trick_req'])} running-id {d['trick_id']} run {d['trick_running']} g{d['trick_group']} d{d['trick_dir']} armed {d['trick_armed']} ground {d['on_ground']} cnt {d['trick_count']}")
        prev = k
print('group-held runs:')
for s, e in runs(lambda d: d['trick_group'] != 0):
    print(f"  {T[s]['t']}-{T[e]['t']} g{sorted({T[i]['trick_group'] for i in range(s, e + 1)})} d{sorted({T[i]['trick_dir'] for i in range(s, e + 1)})} ground {sorted({T[i]['on_ground'] for i in range(s, e + 1)})} held {held(s)}")

print('\n== backflip state changes ==')
prev = None
for i, d in enumerate(T):
    k = (d['flip_on'], d['flip_arm'], d['flip_ramp'])
    if k != prev:
        print(f"  {d['t']}: on {d['flip_on']} arm {d['flip_arm']} ramp {d['flip_ramp']} torqued {d['flip_torqued']} ground {d['on_ground']} spd {d['spd']:.1f} "
              f"ly {d['ly']} torque {d['ft'][0]:.3f},{d['ft'][1]:.3f},{d['ft'][2]:.3f} grace {float(d['flip_grace']):.3f} wreck {d['wreck_reason']}")
        prev = k

print('\n== plain riding (ground, no wreck, no trick, no flip, >= 30 ticks) ==')
for s, e in runs(lambda d: d['wreck_reason'] == 0 and d['trick_running'] == 0 and d['flip_on'] == 0 and d['flip_ramp'] == 0
                 and d['airborne'] == 0 and d['fresh'] == 0):
    if e - s + 1 >= 30:
        sp = [T[i]['spd'] for i in range(s, e + 1)]
        gas = sum(1 for i in range(s, e + 1) if 2 in T[i]['held'])
        brk = sum(1 for i in range(s, e + 1) if 3 in T[i]['held'])
        steer = sum(1 for i in range(s, e + 1) if abs(T[i]['lx'] - 127.5) > 30)
        print(f"  {T[s]['t']}-{T[e]['t']} ({e - s + 1}) spd {sp[0]:.1f}->{sp[-1]:.1f} max {max(sp):.1f} gas {gas} brake {brk} steer {steer}")
print('\n== still (speed < 1, >= 15 ticks) ==')
for s, e in runs(lambda d: d['spd'] < 1.0):
    if e - s + 1 >= 15:
        print(f"  {T[s]['t']}-{T[e]['t']} ({e - s + 1}) wreck {sorted({T[i]['wreck_reason'] for i in range(s, e + 1)})}")
sp = [d['spd'] for d in T]
print('\nspeed max', round(max(sp), 2), 'at', T[sp.index(max(sp))]['t'], 'path', round(sum(math.dist(T[i]['pos'], T[i + 1]['pos']) for i in range(len(T) - 1)), 1))
from collections import Counter
print('held ticks per control', {CTL[c]: n for c, n in sorted(Counter(c for d in T for c in d['held']).items())})
print('ticks with any control held', sum(1 for d in T if d['held']))
