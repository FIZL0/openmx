"""Log the player's engine note once a frame over PINE: its four hands and its state.

usage: hands.py OUT.csv SECONDS

The addresses are first_recording.p2m2_SaveState's (slot 7 here): the note object the oracle test finds
by its class, and the hands its +0x200 table points at.
"""
import os
import sys
import time

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '../../../..'))
from ps2kit.pine import Pine  # noqa: E402

NOTE = 0xdca880
out, seconds = sys.argv[1], float(sys.argv[2])
p = Pine(lock=False)
hands = [p.read32(p.read32(NOTE + 0x200) + 4 * k) for k in range(4)]
end = time.time() + seconds
last = p.frame()
with open(out, 'w') as f:
    f.write('t,frame,grounded,gas,speed,clutch,note,next,head\n')
    t0 = time.time()
    while time.time() < end:
        last = p.next_frame(last, delay=0.002)
        g = p.read32(hands[0])
        gas, speed, clutch = (p.read_f32(a) for a in hands[1:])
        note, nxt = p.read32(NOTE + 0x2d8), p.read32(NOTE + 0x2dc)
        head = p.read32(p.read32(NOTE + 0x4c))
        f.write(f'{time.time() - t0:.4f},{last},{g},{gas:.6g},{speed:.6g},{clutch:.6g},{note},{nxt},{head:#x}\n')
