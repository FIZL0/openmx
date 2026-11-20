"""R22: boot free09 in split screen with player 2's machine a buggy (S148's boot.py, the split event).

usage (on a fresh `./kit pcsx2 start`):  ./kit pcsx2 with -- python3 -u research/r22/boot2.py SLOT [P2KIND]

The user's split state's event (splitscreen2_shortcolision, event 0x00506580): league 2, type 5, "Free09",
+0x10c 1, +0x110 0, +0x114 two human players, +0x115..+0x11b 05 00 5a 01 01 00 00, then per player
(stride 0xc) the machine kind (+0x11c), a flag byte (+0x120) and the skin (+0x124): both biplanes there.
Player 2's kind is P2KIND here (3, the buggy: machine_placement reads [Buggy2]), skin 0. Player 1 keeps
the biplane. Rides eight seconds in, then PINE saves to SLOT.
"""
import sys, time
sys.path.insert(0, '.')
from ps2kit.pine import Pine

slot = int(sys.argv[1])
p2kind = int(sys.argv[2]) if len(sys.argv) > 2 else 3
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

name = b'Free09\0'
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
                p.write32(ev + 0x10c, 1)
                p.write32(ev + 0x110, 0)
                for i, b in enumerate([2, 5, 0, 0x5a, 1, 1, 0, 0]):
                    p.write8(ev + 0x114 + i, b)
                p.write32(ev + 0x11c, 1)
                p.write8(ev + 0x120, 0)
                p.write32(ev + 0x124, 2)
                p.write32(ev + 0x128, p2kind)
                p.write8(ev + 0x12c, 0)
                p.write32(ev + 0x130, 0)
        if p.read32(SCENE):
            break
    except (OSError, RuntimeError):
        pass
    time.sleep(0.02)
print('scene', round(time.time() - t0, 1), flush=True)
end = time.time() + 120
while p.read32(IN_GAME) != 1:
    if time.time() > end:
        raise SystemExit('not in game in 120 s')
    time.sleep(0.1)
print('in game, frame', p.frame(), flush=True)
end = time.time() + 8
while time.time() < end:
    time.sleep(0.25)
print('frame', p.frame(), flush=True)
p.save_state(slot)
time.sleep(2)
print('saved', slot, flush=True)
