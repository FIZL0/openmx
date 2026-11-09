"""C31: boot sx01 as a single race whose riders all ride themselves, and save a state at the start.

S133's scripted boot (research/journal/2026-10-28-s133-hierarchy-toy-parts/boot.py) with three changes:
NumberOfRacers is RACERS instead of 1, the AllAI knob is on (so the player's own bike is ridden by the
AI, and its score card counts real laps with no pad), and the event's lap count is LAPS. It saves the
state the moment the world is running, before the start's countdown ends.

usage: boot.py SLOT [RACERS] [LAPS] [ALL_AI]
Run as `./kit pcsx2 with -- python3 research/c31/boot.py 27` on a freshly started copy.
"""
import sys, time
sys.path.insert(0, '.')
from ps2kit.pine import Pine

slot = int(sys.argv[1])
racers = int(sys.argv[2]) if len(sys.argv) > 2 else 4
laps = int(sys.argv[3]) if len(sys.argv) > 3 else 3
all_ai = int(sys.argv[4]) if len(sys.argv) > 4 else 1
track, league, kind = 'sx01', 0, 2  # supercross, SingleRace
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
    if time.time() - t0 > 400:
        raise SystemExit('no scene in 400 s')
    try:
        s = p.read32(SETTINGS)
        if s:
            ev = p.read32(s + 0xc)
            p.write32(APP_START_MODE, 2)
            p.write32(s, racers)
            p.write32(ALL_AI, all_ai)
            if ev:
                p.write32(ev + 0, league)
                p.write32(ev + 4, kind)
                for i, b in enumerate(name):
                    p.write8(ev + 8 + i, b)
                p.write8(ev + 0x114, 1)
                p.write8(ev + 0x115, laps)
                p.write32(ev + 0x10c, 2)
                p.write32(ev + 0x110, 0)
        if p.read32(SCENE):
            break
    except (OSError, RuntimeError):
        pass
    time.sleep(0.02)
print('scene', round(time.time() - t0, 1), flush=True)
while p.read32(IN_GAME) != 1:
    time.sleep(0.1)
print('in game, frame', p.frame(), flush=True)
p.save_state(slot)
time.sleep(3)
print('saved', slot, 'frame', p.frame(), flush=True)
