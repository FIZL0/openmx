"""MD160: a tipping truck state with no uinput pad. Loads slot IN (an in-air truck state), writes the
rigid body's angular rates (body +0x1d0) once the game runs, and saves the next frame to slot OUT. The
truck lands on its side or roof, and the car's tipped wreck sets the body's dampings to WreckDamping
(obj +0xc90, 0.999): the lowered angular damping Untamed's body integrate takes powf of. Run by
flip.sh, with no capture running (one PINE connection).

  python3 research/md160/flip.py IN CAR RX RY RZ OUT
"""
import os
import struct
import sys
import time

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..'))
from ps2kit.pine import Pine          # noqa: E402

slot, car = int(sys.argv[1]), int(sys.argv[2], 0)
rates, out = [float(x) for x in sys.argv[3:6]], int(sys.argv[6])
p = Pine()
p.load_state(slot)
time.sleep(1)
body = p.read32(car + 0xe8)
f = p.frame()
f = p.next_frame(f)
old = [p.read_f32(body + 0x1d0 + 4 * i) for i in range(3)]
for i, v in enumerate(rates):
    p.write_f32(body + 0x1d0 + 4 * i, v)
f2 = p.next_frame(f)
p.save_state(out)
now = [p.read_f32(body + 0x1d0 + 4 * i) for i in range(3)]
print(f'rates {old} -> {rates} written after vsync {f}; saved slot {out} at vsync {f2}, rates then {now}')
