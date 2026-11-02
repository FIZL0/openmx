"""S228/S227: every touchdown after a jump in the fly_and_collide capture, nose-first ones marked.

    python3 -B research/s228/landings.py [context/capture/s228/fly.csv] | head -80

A jump is a stretch of `p1_airborne` rows at least MIN_AIR long; its touchdown is the first row after
it with a wheel in contact. Nose-first: the front wheel's contact flag is up on that row and the
rear's is not, or the chassis pitch (+0x90, x of p1_ch_euler) is below -NOSE degrees. The verdict is the
wreck reason on any row within AFTER rows of the touchdown, else "rode out". Research, not product.
"""
import csv
import math
import sys

MIN_AIR = 10      # ticks (1/6 s at the state's 60 Hz)
AFTER = 60        # a wreck this soon after the touch is the landing's
NOSE = 5.0        # degrees nose down
path = sys.argv[1] if len(sys.argv) > 1 else 'context/capture/s228/fly.csv'
want = ['frame_counter', 'p1_airborne', 'p1_wf_contact', 'p1_wr_contact', 'p1_wreck_reason', 'p1_ch_euler.x',
        'p1_ch_euler.z', 'p1_ch_speed', 'p1_g_peak.y', 'p1_hang_time', 'p1_ch_vel.y', 'p1_mode']
rows = []
with open(path) as f:
    r = csv.reader(f)
    head = next(r)
    at = [head.index(c) for c in want]
    for line in r:
        if len(line) == len(head):
            rows.append({c: line[k] for c, k in zip(want, at)})
num = lambda r, c: float(r[c]) if r[c] not in ('', None) else float('nan')
air = 0
print('| touch | air ticks | hang s | front | rear | pitch deg | roll deg | vy ft/s | speed | nose-first | verdict |')
print('|---|---|---|---|---|---|---|---|---|---|---|')
for i, r in enumerate(rows):
    if r['p1_airborne'] == '1':
        air += 1
        continue
    if air >= MIN_AIR and r['p1_wreck_reason'] == '0':
        before = rows[i - 1]
        fr, rr = r['p1_wf_contact'], r['p1_wr_contact']
        pitch, roll = math.degrees(num(r, 'p1_ch_euler.x')), math.degrees(num(r, 'p1_ch_euler.z'))
        nose = (fr == '1' and rr != '1') or pitch < -NOSE
        verdict = 'rode out'
        for j in range(i, min(i + AFTER, len(rows))):
            if rows[j]['p1_wreck_reason'] != '0':
                verdict = 'wreck %s at %s' % (rows[j]['p1_wreck_reason'], rows[j]['frame_counter'])
                break
        print('| %s | %d | %.3f | %s | %s | %.2f | %.2f | %.1f | %.1f | %s | %s |' % (
            r['frame_counter'], air, num(before, 'p1_hang_time'), fr, rr, pitch, roll, num(before, 'p1_ch_vel.y'),
            num(r, 'p1_ch_speed'), 'yes' if nose else '', verdict))
    air = 0
