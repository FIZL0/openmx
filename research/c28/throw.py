"""C28: MD37a's throw.py with the gas from a playing tape instead of the uinput pad (root-only here).
Waits until the ATV reaches SPEED ft/s, adds UP ft/s to the rigid body's velocity y (body +0x84) and
saves the next frame to slot OUT. Run by throw.sh while gas.p2m2 plays.

  python3 research/c28/throw.py ATV SPEED UP OUT
"""
import os
import sys
import time

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..'))
from ps2kit.pine import Pine          # noqa: E402

atv, speed, up, out = int(sys.argv[1], 0), float(sys.argv[2]), float(sys.argv[3]), int(sys.argv[4])
p = Pine()
deadline = time.time() + 40
while p.read_f32(atv + 0xc38) < speed:
    if time.time() > deadline:
        sys.exit(f'never reached {speed} ft/s')
    time.sleep(0.01)
body = p.read32(atv + 0xb8)
vy = p.read_f32(body + 0x84)
p.write_f32(body + 0x84, vy + up)
f = p.frame()
p.next_frame(f)
p.save_state(out)
print(f'speed {p.read_f32(atv + 0xc38):.2f} vy {vy:.2f} -> {vy + up:.2f}, saved slot {out} at frame {f}')
