"""R48: boot free09 for one player with the biplane parked as P1's machine and the monster truck as
the second machine, then save.

usage: boot.py SLOT

S149a's boot.py (research/journal/2026-10-29-s149a-car-captures) with the event's second machine kind
(+0x128) set to 4 instead of 6: machine_placement (0x0020f6e0) counts a machine for each of +0x11c and
+0x128 that is not 6, so the track parks [Biplane1] and [Monster2]. The human starts on the bike. Run it
under `KIT_PCSX2=N ./kit pcsx2 with --` right after `./kit pcsx2 start`, while the disc boots.
"""
import sys, time
sys.path.insert(0, '.')
from ps2kit.pine import Pine

slot = int(sys.argv[1])
APP_START_MODE, SETTINGS, SCENE, ALL_AI, IN_GAME = 0x00479d98, 0x00479d64, 0x00479d5c, 0x004e06b8, 0x004e2650

p = None
t0 = time.time()
while p is None:
    try:
        p = Pine(check=False, lock=False, timeout=5.0)
    except SystemExit:
        if time.time() - t0 > 60:
            raise
        time.sleep(0.2)
print('pine up', round(time.time() - t0, 1), flush=True)

name = b'free09\0'
while True:
    if time.time() - t0 > 240:
        raise SystemExit('no scene in 240 s')
    try:
        s = p.read32(SETTINGS)
        if s:
            ev = p.read32(s + 0xc)
            p.write32(APP_START_MODE, 2)
            p.write32(s, 1)
            p.write32(ALL_AI, 0)
            if ev:
                p.write32(ev + 0, 2)
                p.write32(ev + 4, 5)
                for i, b in enumerate(name):
                    p.write8(ev + 8 + i, b)
                p.write8(ev + 0x114, 1)
                p.write32(ev + 0x10c, 2)
                p.write32(ev + 0x110, 0)
                p.write32(ev + 0x11c, 1)
                p.write8(ev + 0x116, 0)
                p.write8(ev + 0x120, 0)
                p.write32(ev + 0x128, 4)
                p.write8(ev + 0x12c, 0)
                p.write32(ev + 0x124, 0)
        if p.read32(SCENE):
            break
    except (OSError, RuntimeError):
        pass
    time.sleep(0.02)
print('scene', round(time.time() - t0, 1), flush=True)
while p.read32(IN_GAME) != 1:
    if time.time() - t0 > 150:
        raise SystemExit('never in game')
    time.sleep(0.1)
print('in game, frame', p.frame(), flush=True)
end = time.time() + 8
while time.time() < end:
    time.sleep(0.25)
print('frame', p.frame(), flush=True)
p.save_state(slot)
time.sleep(2)
print('saved', slot, flush=True)
