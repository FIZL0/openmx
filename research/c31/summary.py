"""C31: what a capture holds, per card: laps counted (with the tick each lap line fired), lap times, the
best lap, ticks off the track, the longest off-track clock, the flags seen, and the manager's flags.

usage: python3 -I research/c31/summary.py context/capture/c31/NAME.csv  (cardK_0 or riderN_0 columns)
"""
import csv, sys

csv.field_size_limit(1 << 24)

rows = list(csv.DictReader(open(sys.argv[1])))
print(len(rows), 'rows, frames', rows[0]['frame_counter'], '..', rows[-1]['frame_counter'])
fc = [int(r['frame_counter']) for r in rows]
gaps = [(a, b) for a, b in zip(fc, fc[1:]) if b != a + 1]
print('gaps', len(gaps), gaps[:8])
print('mgr flags seen', sorted({r['mgr_flags'] for r in rows}, key=int)[:20], 'kind', {r['mgr_kind'] for r in rows},
      'laps', {r['mgr_laps'] for r in rows})
pre = 'rider' if 'rider0_0.laps' in rows[0] else 'card'
k = 0
while '%s%d_0.laps' % (pre, k) in rows[0]:
    c = '%s%d_0.' % (pre, k)
    live = [r for r in rows if r[c + 'laps'] != '']
    if not live:
        break
    laps = []
    for a, b in zip(live, live[1:]):
        if b[c + 'laps'] != a[c + 'laps']:
            laps.append((b['frame_counter'], b[c + 'laps'], b[c + 'lap_time'], b[c + 'best_lap']))
    off = sum(1 for r in live if r[c + 'on_track'] == '0')
    print('card %d id %s: laps %s' % (k, live[0][c + 'id'], laps))
    print('   off-track ticks %d, max off clock %s, on_track values %s, flags %s, place %s' % (
        off, max(float(r[c + 'off_clock']) for r in live), sorted({r[c + 'on_track'] for r in live}),
        sorted({hex(int(r[c + 'flags'])) for r in live}), sorted({r[c + 'place'] for r in live})))
    k += 1
