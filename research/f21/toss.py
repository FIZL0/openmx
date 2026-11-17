"""F21: wake one of free11's bowling pins in the live game and toss it at another, then save the state.

usage: python3 -B research/f21/toss.py LOAD_SLOT PIN VX VY VZ SAVE_SLOT

Every pin is asleep in the boot state (slot 21 of research/journal/2026-10-28-s133-hierarchy-toy-parts/
boot.py Free11 2 5) and the boot has no bike to knock one. So the game wakes it itself: a one-shot
listener goes on the props node's listener list (node +0x48 count, +0x50 array; the walk calls each
listener's vt +8 after the step and takes off one that returns nonzero, oracle.txt S242). Its code, in
RAM zero in every state seen (0x01dc7800), calls the pins' collection's activate (0x003b27a0) and the
pin object's enable (0x0015b790), the two calls a struck listener makes (tests/prop_spring.rs makes
the same two in the interpreter), writes (VX, VY, VZ) ft/s into the pin's body velocity (+0x70), sets
a marker and returns 1. Once the marker is seen the state goes into SAVE_SLOT: the pin awake and
moving, every other pin asleep where the loader stood it.

Run it under `KIT_PCSX2=N ./kit pcsx2 with --`, the game running.
"""

import struct
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(ROOT))
from ps2kit.pine import Pine             # noqa: E402

COLLECTION = 0x00d08680
OBJECTS = [0xd08ae0, 0xd0b200, 0xd0fc70, 0xd12d30, 0xd15df0, 0xd18eb0, 0xd1bf90, 0xd1f050, 0xd22110, 0xd251d0]
NODE = 0x007a3d40
ACTIVATE, ENABLE = 0x003b27a0, 0x0015b790
CAVE = 0x01dc7800
FRAME = 0x00479d04

LOAD, PIN = int(sys.argv[1]), int(sys.argv[2])
V = [float(v) for v in sys.argv[3:6]]
SAVE = int(sys.argv[6])


def li(reg, value):
    """lui/ori of a 32-bit value into a register."""
    return [0x3c000000 | reg << 16 | value >> 16, 0x34000000 | reg << 21 | reg << 16 | value & 0xffff]


def jal(target):
    return [0x0c000000 | (target >> 2) & 0x3ffffff, 0]


A0, T0, T1, = 4, 8, 9
obj = OBJECTS[PIN]
p = Pine()
body = p.read32(obj + 0x110)
marker, fake, vtable, code = CAVE, CAVE + 0x10, CAVE + 0x20, CAVE + 0x40
words = [0x27bdffe0, 0xffbf0000]                      # addiu sp, -32; sd ra, 0(sp)
words += li(A0, COLLECTION) + jal(ACTIVATE)
words += li(A0, obj) + jal(ENABLE)
words += li(T0, body)
for k, v in enumerate(V):
    words += li(T1, struct.unpack('<I', struct.pack('<f', v))[0]) + [0xad090000 | 0x70 + 4 * k]   # sw t1, +0x70+4k(t0)
words += li(T0, marker) + li(T1, 1) + [0xad090000]  # the marker
words += [0xdfbf0000, 0x24020001, 0x03e00008, 0x27bd0020]   # ld ra; li v0, 1; jr ra; addiu sp, 32

p.load_state(LOAD)
time.sleep(3.0)
assert all(p.read32(CAVE + 4 * k) == 0 for k in range(0x40)), 'the cave is not empty'
assert p.read32(body) == 0, 'the pin is awake already'
for k, w in enumerate(words):
    p.write32(code + 4 * k, w)
p.write32(vtable + 8, code)
p.write32(fake, vtable)
n, cap, arr = p.read32(NODE + 0x48), p.read32(NODE + 0x4c), p.read32(NODE + 0x50)
assert n < cap, f'the node list is full ({n} of {cap})'
p.write32(arr + 4 * n, fake)
p.write32(NODE + 0x48, n + 1)
t = time.time()
while p.read32(marker) == 0:
    assert time.time() - t < 5, 'the listener never ran'
f = p.read32(FRAME)
p.save_state(SAVE)
print('frame', f, 'pin', PIN, 'woken: enabled', p.read32(body), 'vel', struct.unpack('<3f', p.read_block(body + 0x70, 12)),
      'collection flags %#x' % p.read32(COLLECTION + 0x20), 'saved', SAVE, flush=True)
time.sleep(2.0)
