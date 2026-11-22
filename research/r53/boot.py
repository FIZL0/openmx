"""R53: R48's boot (research/r48/boot.py: free09, [Biplane1] and [Monster2] parked) for PLAYERS
players (default 2): the event's player count byte +0x114. Then save.

usage: boot.py SLOT [PLAYERS]
"""
import sys, time
sys.path.insert(0, '.')
from ps2kit.pine import Pine

slot = int(sys.argv[1])
players = int(sys.argv[2]) if len(sys.argv) > 2 else 2
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
            p.write32(s, players)       # the front end's 2p free ride has 2 here (s272-137402)
            p.write32(ALL_AI, 0)
            view = p.read32(s + 8)
            if view and players > 1:
                p.write32(view + 4, 2)  # the viewport mode: 2 vertical split, as in s272-137402

            if ev:
                p.write32(ev + 0, 2)
                p.write32(ev + 4, 5)
                for i, b in enumerate(name):
                    p.write8(ev + 8 + i, b)
                p.write8(ev + 0x114, players)
                p.write32(ev + 0x10c, 2)
                p.write32(ev + 0x110, 0)
                p.write32(ev + 0x11c, 1)
                p.write8(ev + 0x116, 0)
                p.write8(ev + 0x120, 0)
                p.write32(ev + 0x128, 4)
                p.write8(ev + 0x12c, 0)
                p.write32(ev + 0x124, 2 if players > 1 else 0)   # 2 in s272-137402
        if p.read32(SCENE):
            break
    except (OSError, RuntimeError):
        pass
    time.sleep(0.02)
print('scene', round(time.time() - t0, 1), flush=True)
while p.read32(IN_GAME) != 1:
    if time.time() - t0 > 400:
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
