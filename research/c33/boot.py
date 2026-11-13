"""C33: boot free09 with four racers (the human and three AI riders) and save a state on the world's
first simulated frames, before the AI riders move.

C31's boot (research/c31/boot.py) with recording2's event words instead of sx01's: league 2, kind 5
(Freestyle), track free09, byte +0x114 = 1 (players), +0x115 = 5 (laps), +0x117 = 90 (the AI
difficulty), +0x110 = 0, read off recording2.p2m2_SaveState.eeMemory.bin. The bike class +0x10c is 2
as in C31, not recording2's 1: with 1 the load stalled with the bar a quarter full, twice.
AllAI is 1 as in C31: with it 0 the Freestyle loading screen never finishes (frame_counter stays 0 or
1, the bar half full -- it waits for a press no pad can give here). With it 1 the human's rider still
has no brain and idles, as C31 found.
Poke from the emulator's first second: a copy that booted a while before the poke never loads the world.

usage: boot.py SLOT [RACERS]
Run as `./kit pcsx2 with -- python3 research/c33/boot.py 3` on a freshly started copy.
"""
import sys, time
sys.path.insert(0, '.')
from ps2kit.pine import Pine

slot = int(sys.argv[1])
racers = int(sys.argv[2]) if len(sys.argv) > 2 else 4
import os
league, kind, name = int(os.environ.get('C33_LEAGUE', 2)), int(os.environ.get('C33_KIND', 5)), b'free09\0'
APP_START_MODE, SETTINGS, SCENE, ALL_AI, IN_GAME = 0x00479d98, 0x00479d64, 0x00479d5c, 0x004e06b8, 0x004e2650
FRAME = 0x00479d04

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

while True:
    if time.time() - t0 > 400:
        raise SystemExit('no scene in 400 s')
    try:
        s = p.read32(SETTINGS)
        if s:
            ev = p.read32(s + 0xc)
            p.write32(APP_START_MODE, 2)
            p.write32(s, racers)
            p.write32(ALL_AI, 1)
            if ev:
                p.write32(ev + 0, league)
                p.write32(ev + 4, kind)
                for i, b in enumerate(name):
                    p.write8(ev + 8 + i, b)
                p.write8(ev + 0x114, 1)
                p.write8(ev + 0x115, 5)
                p.write8(ev + 0x117, 90)
                p.write32(ev + 0x10c, 2)
                p.write32(ev + 0x110, 0)
        if p.read32(SCENE):
            break
    except (OSError, RuntimeError):
        pass
    time.sleep(0.02)
print('scene', round(time.time() - t0, 1), flush=True)
while p.read32(IN_GAME) != 1:
    if time.time() - t0 > 600:
        raise SystemExit('never in game')
    time.sleep(0.02)
print('in game, frame_counter', p.read32(FRAME), flush=True)
# in_game turns 1 while the world still loads, and frame_counter sits at 0 and then 1 until the load
# ends; a state saved then never runs on once loaded (the counter stays at 1). Save on the first frame
# past 1 instead: the first simulated frames, before the AI riders have moved off the spawn.
while p.read32(FRAME) <= 1:
    if time.time() - t0 > 700:
        raise SystemExit('frame_counter never moved')
    time.sleep(0.005)
print('running, frame_counter', p.read32(FRAME), flush=True)
if slot == 0:
    raise SystemExit(0)
p.save_state(slot)
time.sleep(3)
print('saved', slot, 'frame_counter now', p.read32(FRAME), flush=True)
