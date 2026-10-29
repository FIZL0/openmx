"""MD37a: make a mid-air ATV state. Loads STATE, holds gas until the ATV reaches SPEED ft/s, adds UP ft/s
to the rigid body's velocity (body +0x80, the y at +0x84), and saves the next frame to slot OUT.

  KIT_CONFIG=<SLUS-21104 kit.toml> ./kit pcsx2 with -- python3 research/md37a/throw.py ATV STATE SPEED UP OUT
"""
import os
import sys
import time

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..'))
from ps2kit.pine import Pine          # noqa: E402
from ps2kit import pad as kpad        # noqa: E402

atv, state, speed, up, out = int(sys.argv[1], 0), int(sys.argv[2]), float(sys.argv[3]), float(sys.argv[4]), int(sys.argv[5])
p = Pine()
kpad.send(['release'], quiet=True)
p.load_state(state)
time.sleep(0.3)
kpad.send(['down cross'], quiet=True)
deadline = time.time() + 15
while p.read_f32(atv + 0xc38) < speed:
    if time.time() > deadline:
        kpad.send(['release'], quiet=True)
        sys.exit(f'never reached {speed} ft/s')
    time.sleep(0.01)
body = p.read32(atv + 0xb8)
vy = p.read_f32(body + 0x84)
p.write_f32(body + 0x84, vy + up)
f = p.frame()
p.next_frame(f)
p.save_state(out)
kpad.send(['release'], quiet=True)
print(f'speed {p.read_f32(atv + 0xc38):.2f} vy {vy:.2f} -> {vy + up:.2f}, saved slot {out} at frame {f}')
