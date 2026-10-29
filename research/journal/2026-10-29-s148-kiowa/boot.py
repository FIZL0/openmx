"""R7's scripted boot, for any track: poke the event until a scene exists, ride a few seconds, save.

usage: boot.py TRACK LEAGUE TYPE SLOT
"""
import sys, time
sys.path.insert(0, '.')
from ps2kit.pine import Pine

track, league, kind, slot = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), int(sys.argv[4])
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

name = track.encode() + b'\0'
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
                p.write32(ev + 0, league)
                p.write32(ev + 4, kind)
                for i, b in enumerate(name):
                    p.write8(ev + 8 + i, b)
                p.write8(ev + 0x114, 1)
                p.write32(ev + 0x10c, 2)
                p.write32(ev + 0x110, 0)
                p.write32(ev + 0x11c, 2)
                p.write8(ev + 0x116, 0)
                p.write8(ev + 0x120, 0)
                p.write32(ev + 0x128, 6)
                p.write32(ev + 0x124, 0)
        if p.read32(SCENE):
            break
    except (OSError, RuntimeError):
        pass
    time.sleep(0.02)
print('scene', round(time.time() - t0, 1), flush=True)
while p.read32(IN_GAME) != 1:
    time.sleep(0.1)
f = p.frame()
print('in game, frame', f, flush=True)
end = time.time() + 8
while time.time() < end:
    time.sleep(0.25)
print('frame', p.frame(), flush=True)
p.save_state(slot)
time.sleep(2)
print('saved', slot, flush=True)
