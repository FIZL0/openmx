"""Write watch/c16-fly.toml, C16's spec for its flying tape: watch/2p.toml's columns (both riders, the path
cars) plus the plane blocks of watch/biplane.toml for plane 0 (P1 flies it after the pause menu's
Switch To Machine) and a few columns of plane 1.

    python3 -B research/c16/make_spec.py > watch/c16-fly.toml

The plane block is biplane.toml's lines from `plane_type` to `crash_time` with their names unchanged, so
a reader of biplane.csv reads the same names here; its chains start at list entry 0, which is plane 0 in
the 2p state too. Research, not product.
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
FRAMES = int(sys.argv[1]) if len(sys.argv) > 1 else 2812
HEAD = '@game_manager -> 0x34 -> 0x34 -> 0x8 -> 0x0'   # list entry 1: plane 1

two_p = (ROOT / 'watch/2p.toml').read_text().splitlines()
plane = (ROOT / 'watch/biplane.toml').read_text().splitlines()

out = '''# c16-fly.toml — C16's flying tape (context/recordings/c16/fly.p2m2): P1 switches to the biplane from
# the 2p pause menu, taxis through P2's parked bike, takes off, crashes inverted into the ground, is put
# back, and taxis into a fixed object south of the spawn. research/c16/index.md has the events.
#
# Generated: `python3 -B research/c16/make_spec.py > watch/c16-fly.toml` (edit the generator).
# watch/2p.toml's columns as they are (the rider chains: entry 3 is P1's vehicle, 2 P2's; P1's vehicle
# stands parked while P1 flies), then watch/biplane.toml's plane block for list entry 0 (plane 0, P1's
# after the switch: plane_ctl_src 1), then plane 1's place and crash word (`plane1_`).
# Played by research/s228/play_capture.sh with SPEC=watch/c16-fly.toml at NominalScalar 0.05:
#
#   ./kit pcsx2 with -- env SPEC=watch/c16-fly.toml bash research/s228/play_capture.sh PID \\
#       context/recordings/c16/fly.p2m2 137403 %d context/capture/c16/fly.csv
#
# The tape's state holds frame_counter 137402; one tape field a tick at 60 Hz.
''' % FRAMES
start = two_p.index('[watch]')
for line in two_p[start:]:
    if line.startswith('name        = "2p"'):
        line = 'name        = "c16-fly"'
    elif line.startswith('description = '):
        line = 'description = "C16: P1 flies plane 0 into P2, the ground and the scenery; both riders, both planes"'
    elif line.startswith('frames      = '):
        line = 'frames      = %d' % FRAMES
    out += line + '\n'

out += '\n# --- plane 0 (watch/biplane.toml) ' + '-' * 70 + '\n'
first = next(i for i, l in enumerate(plane) if l.startswith('name = "plane_type"')) - 1
last = next(i for i, l in enumerate(plane) if l.startswith('name = "rider_type"')) - 1
out += '\n'.join(plane[first:last]).rstrip() + '\n\n'

out += '# --- plane 1 ' + '-' * 90 + '\n'
for name, off, typ in [('type', '0x0', 'u32'), ('ctl_src', '0x20 -> 0x34', 'u32'),
                       ('body_pos', '0x160 -> 0x60', 'f32x3'), ('body_vel', '0x160 -> 0x70', 'f32x3'),
                       ('euler', '0x5f0', 'f32x3'), ('speed', '0x598', 'f32'), ('crash', '0x3b0', 'u32')]:
    out += '[[field]]\nname = "plane1_%s"\naddr = "%s -> %s"\ntype = "%s"\n\n' % (name, HEAD, off, typ)

names = re.findall(r'^name\s*=\s*"([^"]+)"', out, re.M)
top = [n for n in re.findall(r'^\[\[(?:field|array)\]\]\nname\s*=\s*"([^"]+)"', out, re.M)]
dup = {n for n in top if top.count(n) > 1}
if dup:
    raise SystemExit('names twice: %s' % sorted(dup))
sys.stdout.write(out.rstrip() + '\n')
