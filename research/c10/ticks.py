"""The ticks to save a state at (C10): the tick before every event research/c10/index.py put in events.json.

    python3 -B research/c10/ticks.py            # prints the ticks, space-separated, for play_at.sh
    python3 -B research/c10/ticks.py --list     # one line a tick: the tick and the events it is before

Wrecks, near misses and world contacts start at their frame; ragdolls, tricks, backflips and animations at
`from`. Research, not product.
"""
import json
import sys

ev = json.load(open('research/c10/events.json'))
before = {}
for kind in ('wrecks', 'near', 'contacts', 'trucks', 'ragdoll', 'tricks', 'flips', 'anims'):
    for e in ev.get(kind, []):
        t = e.get('frame', e.get('from'))
        what = e.get('what') or e.get('clause') or e.get('reason') or e.get('id') or ''
        before.setdefault(t - 1, []).append('P%d %s %s' % (e['p'], kind, what))
if '--list' in sys.argv:
    for t in sorted(before):
        print(t, '; '.join(before[t]))
else:
    print(' '.join(str(t) for t in sorted(before)))
