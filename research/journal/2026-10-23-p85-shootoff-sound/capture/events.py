"""P85: line a shootoff capture's sound events up with its Video Capture's audio.

usage: python3 -B research/journal/2026-10-23-p85-shootoff-sound/capture/events.py NAME

Reads context/capture/p85/NAME.csv (watch/p85-edge-sound.toml) and NAME.pcm (the mp4's audio as
48 kHz s16le stereo: `ffmpeg -i NAME.mp4 -vn -ac 2 -ar 48000 -f s16le NAME.pcm`). Prints the ticks
where the sound child's state changes, and the audio's loudness per 30 Hz tick around the boom, so
the boom's onset in the audio can be lined up with the tick the Cannon voice appears on.
"""

import array
import csv
import math
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
name = sys.argv[1]
base = ROOT / 'context' / 'capture' / 'p85' / name
rows = list(csv.DictReader(open(f'{base}.csv')))
print('dt', rows[10]['dt'], 'rows', len(rows))
prev = None
events = {}
for r in rows:
    k = (r['reason'], r['cannon_voice'] != '-1', r['rumble_voice'] != '-1', r['music_paused'])
    if k != prev:
        print(r['frame_counter'], 'reason', k[0], 'cannon', k[1], 'rumble', k[2], 'paused', k[3],
              'rumble_vol', r['rumble_vol'], 'song', r['song0_vol'])
        if prev and k[2] and not prev[2]:
            events.setdefault('rumble', int(r['frame_counter']))
        if prev and k[1] and not prev[1]:
            events.setdefault('cannon', int(r['frame_counter']))
    prev = k
print(events)

pcm = Path(f'{base}.pcm')
if not pcm.exists():
    sys.exit(0)
a = array.array('h')
a.frombytes(pcm.read_bytes())
per = 48000 // 30 * 2
n = len(a) // per
loud = []
for i in range(n):
    s = a[i * per:(i + 1) * per]
    loud.append(math.sqrt(sum(x * x for x in s) / len(s)))
db = [20 * math.log10(max(x, 1.0)) for x in loud]
# The boom: the biggest one-tick rise in loudness.
jump = max(range(1, n), key=lambda i: db[i] - db[i - 1])
print(f'audio: {n} ticks; biggest rise at tick {jump} ({db[jump - 1]:.1f} -> {db[jump]:.1f} dB)')
for i in range(max(0, jump - 70), min(n, jump + 30)):
    print(i, f'{db[i]:.1f}')
if 'cannon' in events and 'rumble' in events:
    gap = events['cannon'] - events['rumble']
    print(f'capture: rumble starts {gap} ticks before the cannon; audio tick of the rumble '
          f'should be {jump - gap}')
