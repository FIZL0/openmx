"""C16: the switch's start state. Copies fly_and_collide's state into the kit copy's slot 8, loads it,
sets the cheat flag the pause menu's machine list checks (read32(0x004e2778)+0x5c, S148/P103; without it
"Switch To Machine" is greyed) and saves slot 7. Run under `./kit pcsx2 with --` with KIT_PCSX2 set.

usage: setup_state.py SAVESTATES_DIR
"""
import os, shutil, sys, time
sys.path.insert(0, '.')
from ps2kit.pine import Pine

dest = sys.argv[1]
src = os.path.join('context', 'recordings', 'fly_and_collide.p2m2_SaveState.p2s')
shutil.copyfile(src, os.path.join(dest, 'SLUS-20767 (9866FE54).08.p2s'))
p = Pine(check=False, lock=False, timeout=5.0)
p.load_state(8)
time.sleep(3)
flag = p.read32(0x004e2778) + 0x5c
p.write32(flag, 1)
time.sleep(0.5)
print('cheat flag at', hex(flag), '=', p.read32(flag), flush=True)
p.save_state(7)
time.sleep(3)
print('saved slot 7', flush=True)
