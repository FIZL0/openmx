"""Index the 2p capture (C10): wrecks, near misses, contacts, trucks, ragdolls and animations.

    python3 -B research/c10/index.py [csv-name] [--json]     # context/capture/2p/<csv-name>.csv, default 2p

Prints the journal's tables (markdown). The record addresses are the 2p run's own (research/c10/
records.py on c10/2p-start, constant for the run). Research, not product.
"""
import csv
import json
import math
import os
import sys

name = next((a for a in sys.argv[1:] if not a.startswith('--')), '2p')
rows = [r for r in csv.DictReader(open(os.path.join('context', 'capture', '2p', name + '.csv'))) if None not in r.values()]

# research/c10/records.py on the 2p start state
RIDER_REC = {1: 0x01033d70, 2: 0x00dc24b0}
PLANE_REC = {0x0061cab0: 'biplane 0', 0x01a6f720: 'biplane 1'}
CARS = [0x00febf40, 0x01b7fda0, 0x01b8f300, 0x01b93ef0, 0x01ba0a00, 0x01ba5610]
CAR_COL = {0x00ffa260: 0, 0x01b8c230: 1, 0x01b904c0: 2, 0x01b9f380: 3, 0x01ba1c70: 4, 0x01ba6d20: 5}
# The vehicle's wreck limits (+0x754.., Race500P.phy, both riders): ft/s and radians
LIMIT = {'front': 18.3333, 'back': 21.9999, 'side': 19.0666, 'down': 65.9997, 'gpitch': 1.2217,
         'apitch_down': 1.3963, 'apitch_up': 1.4835, 'aroll': 1.4835}
CLIPS = ['rider_Taunt', 'rider_lazyboy', 'rider_cancan', 'rider_catwalk', 'rider_nohandedlander', 'rider_pendulum',
         'rider_nacnac', 'rider_flatliner', 'rider_hartattack', 'rider_indianair', 'rider_barhop', 'rider_supercan',
         'rider_kod', 'rider_superman', 'rider_saranwrap', 'rider_stalelin', 'rider_tsunami',
         'rider_onehandindianair', 'rider_shoalin', 'rider_cordova', 'rider_deadbody', 'rider_rigamortis',
         'rider_supercanindianair', 'rider_stripper', 'turn_stop_Fwd', 'turn_stop_Mid', 'turn_stop_Bck',
         'turn_still_Fwd', 'turn_still_Mid', 'turn_still_Bck', 'turn_riding_Fwd', 'turn_riding_Mid',
         'turn_riding_Bck', 'reverse_l', 'reverse_r', 'air_Fwd', 'air_Mid', 'air_Bck', 'backflip',
         'nohandedlander_left_right', 'wreck_air_flail', 'hold_head', 'hold_elbow_left', 'hold_elbow_right',
         'hold_knee_left', 'hold_knee_right', 'get_ready_breathe'] + ['get_ready_idle_%02d' % i for i in range(1, 11)] + \
        ['get_ready_start_pos_%02d' % i for i in range(1, 4)]
REASON = {1: 'Front', 2: 'Left', 3: 'Right', 4: 'Back', 5: 'Flat', 6: 'Scenery', 7: 'Joust', 8: 'NoseDive',
          11: 'Tail', 12: 'NoHander'}


def u(r, c):
    return int(rows[r][c])


def fl(r, c):
    return float(rows[r][c])


def v3(r, c):
    return tuple(fl(r, '%s.%s' % (c, k)) for k in 'xyz')


def word(r, c, off):
    b = bytes.fromhex(rows[r][c].replace(' ', ''))
    return int.from_bytes(b[off:off + 4], 'little')


def fc(r):
    return u(r, 'frame_counter')


def speed(r, p):
    return math.sqrt(sum(x * x for x in v3(r, 'p%d_ch_vel' % p)))


def pos(r, p):
    return tuple(fl(r, 'p%d_bike_xform.m3%d' % (p, k)) for k in range(3))


def fwd(r, p):
    return tuple(fl(r, 'p%d_bike_xform.m2%d' % (p, k)) for k in range(3))


def heading(r, p):
    f = fwd(r, p)
    return math.degrees(math.atan2(f[0], f[2]))


def has(c):
    return c in rows[0]


def euler(r, p):
    if has('p%d_ch_euler.x' % p):
        e = v3(r, 'p%d_ch_euler' % p)
        return e[0], e[2]
    f = fwd(r, p)
    return math.asin(max(-1, min(1, f[1]))), float('nan')


def entries(r, p):
    return [tuple(rows[r]['p%d_c%d.%s' % (p, k, f)] for f in ('kind', 'other', 'end.x', 'end.y', 'end.z', 'normal.x',
                                                              'normal.y', 'normal.z', 'point.x', 'point.y', 'point.z'))
            for k in range(11)]


def new_hits(r, p):
    """(kind, other, end y) of entries that changed since the row before."""
    if r == 0:
        return []
    now, before = entries(r, p), entries(r - 1, p)
    return [(int(now[k][0]), int(now[k][1]), float(now[k][3]), tuple(float(x) for x in now[k][5:8]))
            for k in range(11) if now[k] != before[k]]


def other_name(o, p):
    q = 3 - p
    if o == 0:
        return 'scenery'
    if o == RIDER_REC[q]:
        return 'P%d' % q
    if o == RIDER_REC[p]:
        return 'self'
    if o in PLANE_REC:
        return PLANE_REC[o]
    if o in CAR_COL:
        return 'car%d' % CAR_COL[o]
    return 'rec %08x' % o


def stretches(pred):
    out, start = [], None
    for r in range(len(rows)):
        if pred(r) and start is None:
            start = r
        if not pred(r) and start is not None:
            out.append((start, r - 1))
            start = None
    if start is not None:
        out.append((start, len(rows) - 1))
    return out


def g(r, p):
    return tuple(fl(r, 'p%d_g_peak.%s' % (p, k)) for k in 'xyz')


events = {'rows': len(rows), 'first': fc(0), 'wrecks': [], 'contacts': [], 'trucks': [], 'ragdoll': [], 'tricks': [],
          'flips': []}

print('rows %d, frames %d..%d' % (len(rows), fc(0), fc(len(rows) - 1)))
gaps = [fc(r) for r in range(1, len(rows)) if fc(r) != fc(r - 1) + 1]
print('gaps', gaps[:10])
for p in (1, 2):
    bad = [fc(r) for r in range(len(rows)) if u(r, 'p%d_type' % p) != 2 or u(r, 'p%d_index' % p) != p - 1
           or u(r, 'p%d_bike_type' % p) != 4 or u(r, 'p%d_bike_player' % p) != p - 1]
    print('P%d guard misses %d %s' % (p, len(bad), bad[:5]))


def clause(r, p, reason):
    """The wreck test's clause that answered `reason` (air.rs wreck_test_in, in its order), the quantity it
    judged and its value. The loads a row shows are the tick's peak; the crossup is not captured."""
    gx, gy, gz = g(r, p)
    pitch, roll = euler(r, p)
    air = u(r, 'p%d_airborne' % p)
    hits = [(k, other_name(o, p), round(ey, 2)) for k, o, ey, _ in new_hits(r, p)]
    deg = '%.2f deg'
    if reason == 4 and gz > LIMIT['back']:
        return 'fore-aft g > BackWreckSpeedLimit', 'g.z %.2f (limit 22.0)' % gz, hits
    if reason == 1 and gz < -LIMIT['front']:
        return 'fore-aft g < -FrontWreckSpeedLimit', 'g.z %.2f (limit -18.33)' % gz, hits
    if reason in (2, 3) and abs(gx) > LIMIT['side']:
        return 'side g past SideWreckSpeedLimit', 'g.x %.2f (limit 19.07)' % gx, hits
    if reason == 5:
        return 'vertical g > FreestyleDownWreckSpeedLimit', 'g.y %.2f (limit 66.0)' % gy, hits
    if reason == 1:
        return 'ground pitch down', ('pitch ' + deg + ' (limit -70)') % math.degrees(pitch), hits
    if reason == 11 and not air and pitch > LIMIT['gpitch']:
        return 'ground pitch up', ('pitch ' + deg + ' (limit 70)') % math.degrees(pitch), hits
    if reason == 8:
        which = 'roll' if abs(roll) > LIMIT['aroll'] else 'pitch down'
        return 'airborne %s' % which, ('pitch ' + deg + ' (limit -80), roll ' + deg + ' (limit +-85)') % (
            math.degrees(pitch), math.degrees(roll)), hits
    if reason == 11 and (pitch > LIMIT['apitch_up'] or abs(roll) > LIMIT['aroll']):
        return 'airborne pitch up / roll', ('pitch ' + deg + ' (limit 85), roll ' + deg + ' (limit +-85)') % (
            math.degrees(pitch), math.degrees(roll)), hits
    if reason == 11:
        return 'backflip torque on a touch', 'flip_torqued %s, pitch %.2f deg' % (
            rows[r]['p%d_flip_torqued' % p], math.degrees(pitch)), hits
    if reason in (2, 3):
        return 'landing crossup', 'hang time %.3f s (crossup itself not captured)' % fl(r, 'p%d_hang_time' % p) \
            if has('p%d_hang_time' % p) else 'hang time not captured', hits
    if reason == 7:
        return 'joust (rider-rider)', 'g %s' % str((round(gx, 2), round(gy, 2), round(gz, 2))), hits
    if reason == 6:
        return 'a body ray on scenery', 'g %s' % str((round(gx, 2), round(gy, 2), round(gz, 2))), hits
    if reason == 12:
        tid = u(r - 1, 'p%d_trick_id' % p)
        return 'landed inside a trick (trick ending)', 'trick %d %s still running' % (tid, CLIPS[tid] if tid < 60 else ''), hits
    return '?', 'g %s' % str((round(gx, 2), round(gy, 2), round(gz, 2))), hits


print('\n## Wrecks\n')
print('| frame | row | rider | reason | clause | value | speed before | new entries (kind, other, ray end y) |')
print('|---|---|---|---|---|---|---|---|')
for p in (1, 2):
    col = 'p%d_wreck_reason' % p
    for r in range(1, len(rows)):
        if u(r, col) != 0 and u(r - 1, col) == 0:
            reason = u(r, col)
            # the judged quantities may sit in the row before (rows read at three phases)
            c, val, hits = clause(r, p, reason)
            c0, val0, _ = clause(r - 1, p, reason)
            events['wrecks'].append({'p': p, 'frame': fc(r), 'row': r, 'reason': reason})
            print('| %d | %d | P%d | %d %s | %s | %s (row before: %s) | %.1f | %s |' % (
                fc(r), r, p, reason, REASON.get(reason, '?'), c, val, val0, speed(r - 1, p), hits[:4]))

print('\n## Near misses (ridden out: the biggest loads and attitudes each clause judged that did not wreck)\n')
print('Riding rows only: no wreck reason within 10 ticks either side and body mode 1 for 10 ticks before (a')
print('put-back teleports the bike). The g clauses read the tick\'s peak g; ground pitch reads grounded rows;')
print('the airborne pitch and roll limits are judged on a touch, so they read the landing tick (airborne the')
print('row before, not now). A row is a near miss when it reaches half its limit.\n')
print('| rider | clause | limit | frame | row | value | share of limit |')
print('|---|---|---|---|---|---|---|')
events['near'] = []
for p in (1, 2):
    wr = [u(r, 'p%d_wreck_reason' % p) for r in range(len(rows))]
    md = [u(r, 'p%d_mode' % p) for r in range(len(rows))]
    air = [u(r, 'p%d_airborne' % p) for r in range(len(rows))]
    riding = [r for r in range(10, len(rows) - 10) if not any(wr[r - 10:r + 11]) and all(m == 1 for m in md[r - 10:r + 1])]
    ground = [r for r in riding if not air[r]]
    touch = [r for r in ground if air[r - 1]]
    for lab, key, lim, among in (
            ('fore-aft g (front)', lambda r: -g(r, p)[2], LIMIT['front'], riding),
            ('fore-aft g (back)', lambda r: g(r, p)[2], LIMIT['back'], riding),
            ('side g', lambda r: abs(g(r, p)[0]), LIMIT['side'], riding),
            ('vertical g (flat)', lambda r: g(r, p)[1], LIMIT['down'], riding),
            ('ground pitch down (deg)', lambda r: -math.degrees(euler(r, p)[0]), math.degrees(LIMIT['gpitch']), ground),
            ('ground pitch up (deg)', lambda r: math.degrees(euler(r, p)[0]), math.degrees(LIMIT['gpitch']), ground),
            ('touch pitch down (deg)', lambda r: -math.degrees(euler(r, p)[0]), math.degrees(LIMIT['apitch_down']), touch),
            ('touch pitch up (deg)', lambda r: math.degrees(euler(r, p)[0]), math.degrees(LIMIT['apitch_up']), touch),
            ('touch |roll| (deg)', lambda r: abs(math.degrees(euler(r, p)[1])), math.degrees(LIMIT['aroll']), touch)):
        best = []
        for r in sorted(among, key=key, reverse=True):
            if key(r) < lim / 2 or len(best) == 3:
                break
            if all(abs(r - s) > 15 for s in best):
                best.append(r)
        if not best:
            top = max(among, key=key)
            print('| P%d | %s | %.2f | (best %d) | %d | %.2f | %.0f%% |' % (p, lab, lim, fc(top), top, key(top),
                                                                        100 * key(top) / lim))
        for r in best:
            events['near'].append({'p': p, 'frame': fc(r), 'row': r, 'clause': lab, 'value': round(key(r), 3),
                                   'limit': round(lim, 3)})
            print('| P%d | %s | %.2f | %d | %d | %.2f | %.0f%% |' % (p, lab, lim, fc(r), r, key(r), 100 * key(r) / lim))


def onsets(p):
    """(row, names, hits) where rider p's record starts naming something it did not the row before."""
    prev = set()
    for r in range(1, len(rows)):
        hits = new_hits(r, p)
        names = set(other_name(o, p) for _, o, _, _ in hits)
        onset = names - prev
        prev = names
        if onset:
            yield r, onset, hits


def wreck_after(r, p, within=5):
    return next((fc(k) for k in range(r, min(r + within, len(rows))) if u(k, 'p%d_wreck_reason' % p) and
                 not u(k - 1, 'p%d_wreck_reason' % p)), '')


def rr_kind(r, p):
    """S88's kinds of rider-vs-rider contact, judged from the row before: into a wrecked rider, landing
    on (this rider airborne and above), head-on (headings more than 135 deg apart), else a side swipe."""
    q = 3 - p
    if u(r - 1, 'p%d_wreck_reason' % q):
        return 'into a wrecked rider'
    if u(r - 1, 'p%d_airborne' % p) and pos(r - 1, p)[1] > pos(r - 1, q)[1] + 1.0:
        return 'landing on'
    if u(r - 1, 'p%d_airborne' % q) and pos(r - 1, q)[1] > pos(r - 1, p)[1] + 1.0:
        return 'landed on'
    d = abs((heading(r - 1, p) - heading(r - 1, q) + 180) % 360 - 180)
    return 'head-on' if d > 135 else 'side swipe' if d > 45 else 'from behind'


print('\n## World contacts (onsets of record entries on scenery or a record that is no rider, plane or car)\n')
print('| frame | row | rider | names | kinds | ray end y | normal | speed | wreck after |')
print('|---|---|---|---|---|---|---|---|---|')
for p in (1, 2):
    for r, onset, hits in onsets(p):
        world = sorted(n for n in onset if n == 'scenery' or n.startswith('rec '))
        if not world:
            continue
        hs = [h for h in hits if other_name(h[1], p) in world]
        events['contacts'].append({'p': p, 'frame': fc(r), 'row': r, 'names': world})
        print('| %d | %d | P%d | %s | %s | %.2f | %s | %.1f | %s |' % (
            fc(r), r, p, ', '.join(world), sorted(set(k for k, _, _, _ in hs)), max(e for _, _, e, _ in hs),
            tuple(round(x, 2) for x in hs[0][3]), speed(r - 1, p), wreck_after(r, p)))

print('\n## Rider-vs-rider contacts\n')
print('| frame | row | rider | other | kind | kinds | ray end y | speed | other speed | height diff | wreck after (self, other) |')
print('|---|---|---|---|---|---|---|---|---|---|---|')
for p in (1, 2):
    q = 3 - p
    for r, onset, hits in onsets(p):
        if 'P%d' % q not in onset:
            continue
        hs = [h for h in hits if other_name(h[1], p) == 'P%d' % q]
        kind = rr_kind(r, p)
        events['contacts'].append({'p': p, 'frame': fc(r), 'row': r, 'names': ['P%d' % q], 'kind': kind})
        print('| %d | %d | P%d | P%d | %s | %s | %.2f | %.1f | %.1f | %.2f | %s, %s |' % (
            fc(r), r, p, q, kind, sorted(set(k for k, _, _, _ in hs)), max(e for _, _, e, _ in hs),
            speed(r - 1, p), speed(r - 1, q), pos(r - 1, p)[1] - pos(r - 1, q)[1], wreck_after(r, p),
            wreck_after(r, q)))

print('\n## Contacts with other actors (biplanes, path cars by their collidable)\n')
print('| frame | row | rider | names | kinds | ray end y | speed | wreck after |')
print('|---|---|---|---|---|---|---|---|')
for p in (1, 2):
    for r, onset, hits in onsets(p):
        other = sorted(n for n in onset if n.startswith('biplane') or n.startswith('car') or n == 'self')
        if not other:
            continue
        hs = [h for h in hits if other_name(h[1], p) in other]
        events['contacts'].append({'p': p, 'frame': fc(r), 'row': r, 'names': other})
        print('| %d | %d | P%d | %s | %s | %.2f | %.1f | %s |' % (
            fc(r), r, p, ', '.join(other), sorted(set(k for k, _, _, _ in hs)), max(e for _, _, e, _ in hs),
            speed(r - 1, p), wreck_after(r, p)))

print('\n## Truck rides (a wheel on a path car)\n')
print('| from | to | rows | rider | car | car speed | wheels | stays on |')
print('|---|---|---|---|---|---|---|---|')
for p in (1, 2):
    def on(r):
        return any(u(r, 'p%d_%s_car' % (p, w)) in CARS for w in ('wf', 'wr'))
    for a, b in stretches(on):
        cars = sorted(set(CARS.index(u(r, 'p%d_%s_car' % (p, w))) for r in range(a, b + 1) for w in ('wf', 'wr')
                          if u(r, 'p%d_%s_car' % (p, w)) in CARS))
        c = cars[0]
        # the car's own velocity word reads 0: its speed is the step, (pose - prev) * 60
        sp = max(60 * math.dist([fl(r, 'car%d_xform.m3%d' % (c, k)) for k in range(3)], v3(r, 'car%d_prev' % c))
                 for r in range(a, b + 1))
        wheels = sorted(set(w for r in range(a, b + 1) for w in ('wf', 'wr') if u(r, 'p%d_%s_car' % (p, w)) in CARS))
        wreck = any(u(r, 'p%d_wreck_reason' % p) for r in range(a, b + 1))
        events['trucks'].append({'p': p, 'from': fc(a), 'to': fc(b), 'cars': cars})
        print('| %d | %d | %d | P%d | %s | %.1f | %s | %s |' % (fc(a), fc(b), b - a + 1, p, cars, sp, wheels,
                                                              'wrecked' if wreck else 'yes'))

print('\n## Ragdoll (body mode 2) and the put-back\n')
print('| rider | mode 2 from | to | wreck reason | reason clears | rows |')
print('|---|---|---|---|---|---|')
for p in (1, 2):
    for a, b in stretches(lambda r: int(rows[r]['p%d_mode' % p]) == 2):
        clear = next((fc(k) for k in range(a, len(rows)) if u(k, 'p%d_wreck_reason' % p) == 0), None)
        events['ragdoll'].append({'p': p, 'from': fc(a), 'to': fc(b)})
        print('| P%d | %d | %d | %d | %s | %d |' % (p, fc(a), fc(b), u(a, 'p%d_wreck_reason' % p), clear, b - a + 1))

print('\n## Body modes (all stretches that are not 1)\n')
for p in (1, 2):
    out = []
    m = [int(rows[r]['p%d_mode' % p]) for r in range(len(rows))]
    for k in (0, 3, 4):
        out += [(fc(a), fc(b), k) for a, b in stretches(lambda r: m[r] == k)]
    print('P%d: %s' % (p, ', '.join('%d %d..%d' % (k, a, b) for a, b, k in sorted(out))))

print('\n## Tricks (trick_running) and backflips (flip_on)\n')
for p in (1, 2):
    for a, b in stretches(lambda r: u(r, 'p%d_trick_running' % p) != 0):
        tid = u(a, 'p%d_trick_id' % p)
        st = word(a + 1 if a + 1 < len(rows) else a, 'p%d_d_cross' % p, 0x14)
        events['tricks'].append({'p': p, 'from': fc(a), 'to': fc(b), 'id': tid})
        landed = next((u(k, 'p%d_wreck_reason' % p) for k in range(b, min(b + 10, len(rows)))
                       if u(k, 'p%d_wreck_reason' % p)), 0)
        print('P%d trick %d..%d id %d machine state %d %s %s' % (
            p, fc(a), fc(b), tid, st, CLIPS[st] if 0 <= st < 60 else '', 'wrecked %d' % landed if landed else ''))
    for a, b in stretches(lambda r: u(r, 'p%d_flip_on' % p) != 0):
        events['flips'].append({'p': p, 'from': fc(a), 'to': fc(b)})
        print('P%d backflip fired %d..%d' % (p, fc(a), fc(b)))

print('\n## Machine states (clip rows) other than riding/air, per rider\n')
for p in (1, 2):
    st = [word(r, 'p%d_d_cross' % p, 0x14) for r in range(len(rows))]
    out = []
    for s in sorted(set(st)):
        runs = stretches(lambda r: st[r] == s)
        out.append('%s (%d): %d runs, %d rows; %s' % (CLIPS[s] if 0 <= s < 60 else s, s, len(runs),
                                                      sum(b - a + 1 for a, b in runs),
                                                      ' '.join('%d..%d' % (fc(a), fc(b)) for a, b in runs[:12])))
    print('P%d:\n  ' % p + '\n  '.join(out))

print('\n## Animations: foot down, reverse, put-backs, lean extremes\n')
events['anims'] = []
for p in (1, 2):
    st = [word(r, 'p%d_d_cross' % p, 0x14) for r in range(len(rows))]
    for lab, ids in (('foot down (turn_stop/turn_still)', range(24, 30)), ('reverse', (33, 34)),
                     ('backflip clip', (38,)), ('no-handed lander', (39,)), ('wreck flail', (40,)),
                     ('hold (after a wreck)', range(41, 46))):
        for a2, b2 in stretches(lambda r: st[r] in ids):
            clips = sorted(set(CLIPS[st[r]] for r in range(a2, b2 + 1)))
            events['anims'].append({'p': p, 'what': lab, 'from': fc(a2), 'to': fc(b2), 'clips': clips})
            print('P%d %-34s %d..%d (%d rows) %s' % (p, lab, fc(a2), fc(b2), b2 - a2 + 1, ', '.join(clips)))
    m = [int(rows[r]['p%d_mode' % p]) for r in range(len(rows))]
    for a2, b2 in stretches(lambda r: m[r] == 0):
        events['anims'].append({'p': p, 'what': 'put-back (mode 0)', 'from': fc(a2), 'to': fc(b2)})
        print('P%d %-34s %d..%d (%d rows) after mode %d' % (p, 'put-back (body mode 0)', fc(a2), fc(b2), b2 - a2 + 1,
                                                            m[a2 - 1] if a2 else -1))
    if has('p%d_ch_euler.z' % p):
        ground = [r for r in range(len(rows)) if m[r] == 1 and not u(r, 'p%d_airborne' % p)
                  and not u(r, 'p%d_wreck_reason' % p)]
        for side, sign in (('lean left (roll min)', 1), ('lean right (roll max)', -1)):
            top = sorted(ground, key=lambda r: sign * fl(r, 'p%d_ch_euler.z' % p))
            seen = []
            for r in top:
                if all(abs(r - k) > 30 for k in seen):
                    seen.append(r)
                if len(seen) == 3:
                    break
            for r in seen:
                events['anims'].append({'p': p, 'what': side, 'from': fc(r), 'to': fc(r),
                                        'roll': fl(r, 'p%d_ch_euler.z' % p)})
            print('P%d %-34s %s' % (p, side, ', '.join('%d (row %d) %.1f deg at %.1f' % (
                fc(r), r, math.degrees(fl(r, 'p%d_ch_euler.z' % p)), speed(r, p)) for r in seen)))

if '--json' in sys.argv:
    json.dump(events, open(os.path.join('research', 'c10', 'events.json'), 'w'), indent=1)
