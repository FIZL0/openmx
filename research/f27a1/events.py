"""F27a1: the knock capture's events, a line each: ball 2's wake and every tick its fall turns (a bounce,
which its own probe hears as a material-21 hit), with the bike's and the ball's speeds.

usage: python3 -I research/f27a1/events.py DIR NAME [BALL]

DIR/NAME.csv is research/s195/knock.py's (watch/s195-knock.toml). Prints every tick in the wake's ±3 too."""
import csv
import math
import sys

rows = list(csv.DictReader(open(f'{sys.argv[1]}/{sys.argv[2]}.csv')))
b = int(sys.argv[3]) if len(sys.argv) > 3 else 2


def speed(row, k):
    return math.hypot(*(float(row[k + '.' + a]) for a in 'xyz'))


wake = next(int(r['frame']) for r in rows if r[f'b{b}_en'] != '0')
pvy = None
for r in rows:
    f = int(r['frame'])
    vy = float(r[f'b{b}_vel.y'])
    bounce = pvy is not None and pvy < -1.0 and vy > pvy + 1.0
    if abs(f - wake) <= 3 or bounce:
        print('%d %-6s bike %6.2f ft/s  ball %6.2f ft/s  vy %6.2f  y %7.2f  touch %s  en %s' % (
            f, 'wake' if f == wake else 'bounce' if bounce else '', speed(r, 'ch_vel'), speed(r, f'b{b}_vel'),
            vy, float(r[f'b{b}_pos.y']), r[f'b{b}_touch'], ''.join(r[f'b{k}_en'] for k in range(5))))
    pvy = vy
