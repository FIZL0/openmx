"""F27a1: pick the port's run-up to free09's ball 2 so the bike meets it at the original's knock speed.

usage: python3 -I research/f27a1/tune.py OPENMX WANT_FT_S [DIST...]

For each start DIST ft short of ball 2 on the original's aimed line (research/s195/aim.py's: heading
35.88 deg) and each count of full-gas ticks from a stop, the shell's `--trace --trace-feel` (the bike
alone, no props) is stepped and the bike's 3-D speed read on the first tick its body comes within NEAR
ft of the ball's centre on the ground plane. Prints the closest few to WANT_FT_S."""
import math
import subprocess
import sys

BALL = (5042.84, 3554.94)                  # ball 2's body, as aim.py printed it (S195 and F27a1 alike)
FWD = (0.586, 0.810)
DEG = math.degrees(math.atan2(*FWD))
NEAR = 7.0

exe, want = sys.argv[1], float(sys.argv[2])
dists = [float(d) for d in sys.argv[3:]] or [40.0, 50.0, 60.0]
n = math.hypot(*FWD)
fx, fz = FWD[0] / n, FWD[1] / n
out = []
for d in dists:
    x, z = BALL[0] - d * fx, BALL[1] - d * fz
    for gas in range(4, 16):
        r = subprocess.run([exe, '--track', 'free09', '--trace', '120', '--trace-at', f'{x:.2f},{z:.2f},{DEG:.2f}',
                            '--trace-switch', str(gas), '--trace-feel'], capture_output=True, text=True, timeout=300)
        for line in r.stdout.splitlines():
            w = line.split()
            if len(w) < 12 or not w[0].isdigit():
                continue
            vx, vy, vz, px, pz = (float(w[k]) for k in (2, 3, 4, 5, 7))
            if math.hypot(px - BALL[0], pz - BALL[1]) < NEAR:
                out.append((abs(math.sqrt(vx * vx + vy * vy + vz * vz) - want), d, gas, int(w[0]),
                            math.sqrt(vx * vx + vy * vy + vz * vz), math.hypot(px - BALL[0], pz - BALL[1])))
                break
for e, d, gas, tick, v, off in sorted(out)[:8]:
    print(f'start {d:5.1f} ft  gas {gas:2d} ticks  near on tick {tick:3d}  {v:6.2f} ft/s  {off:4.1f} ft off')
