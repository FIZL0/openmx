"""Write watch/n24-meet.toml, N24's spec for its two-plane tape: watch/c16-fly.toml's head (frame, dt, rand,
both pads) and its plane-0 block (watch/biplane.toml's, list entry 0, P1's plane) twice: as it is, and for
list entry 1 (P2's plane) with every name prefixed `plane1_`. No rider columns: both players fly.

    python3 -B research/n24/make_spec.py FRAMES > watch/n24-meet.toml

Research, not product.
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
FRAMES = int(sys.argv[1])
c16 = (ROOT / 'watch/c16-fly.toml').read_text()
head = c16[c16.index('[watch]'):c16.index('# ' + '-' * 100 + '\n# P1:')]
block = c16[c16.index('# --- plane 0'):c16.index('# --- plane 1')]
block = block[block.index('\n') + 1:]
head = re.sub(r'name        = "c16-fly"', 'name        = "n24-meet"', head)
head = re.sub(r'description = ".*"', 'description = "N24: P1 and P2 both fly; P1 taxis plane 0 into plane 1"', head)
head = re.sub(r'frames      = \d+', 'frames      = %d' % FRAMES, head)
one = block.replace('@game_manager -> 0x34 -> 0x34 -> 0x0 ->', '@game_manager -> 0x34 -> 0x34 -> 0x8 -> 0x0 ->')
assert one.count('0x8 -> 0x0 ->') == block.count('0x34 -> 0x34 -> 0x0 ->') > 0
one = re.sub(r'^name(\s*)= "', r'name\1= "plane1_', one, flags=re.M)
# array members keep their names: only the top-level [[field]]/[[array]] names are prefixed
one = re.sub(r'(\[\[array\.field\]\]\n)name(\s*)= "plane1_', r'\1name\2= "', one)
out = '''# n24-meet.toml — N24's two-plane tape (context/recordings/n24/meet.p2m2): from C16's fly state (P1 in
# plane 0), P2 switches to plane 1 from its own pause menu, then P1 taxis plane 0 into plane 1's tail and
# pushes it along. research/n24/index.md has the events.
#
# Generated: `python3 -B research/n24/make_spec.py %d > watch/n24-meet.toml` (edit the generator).
# Played by research/s228/play_capture.sh with SPEC=watch/n24-meet.toml at NominalScalar 0.05.
# The tape's state holds frame_counter 137402; one tape field a tick at 60 Hz.

''' % FRAMES + head + '# --- plane 0 (list entry 0: P1\'s) ' + '-' * 60 + '\n' + block.rstrip() + '\n\n' \
    + '# --- plane 1 (list entry 1: P2\'s) ' + '-' * 60 + '\n' + one.rstrip() + '\n'
top = re.findall(r'^\[\[(?:field|array)\]\]\nname\s*=\s*"([^"]+)"', out, re.M)
dup = {n for n in top if top.count(n) > 1}
if dup:
    raise SystemExit('names twice: %s' % sorted(dup))
sys.stdout.write(out)
