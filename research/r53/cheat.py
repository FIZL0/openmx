"""R53: load LOAD (the 2p boot), set the cheat flag the pause menu's machine list checks
(read32(0x004e2778)+0x5c, as research/c16/setup_state.py) and save SAVE.

usage: cheat.py LOAD SAVE     (under KIT_PCSX2=N ./kit pcsx2 with --)
"""
import sys, time
sys.path.insert(0, '.')
from ps2kit.pine import Pine

p = Pine(check=False, lock=False, timeout=5.0)
p.load_state(int(sys.argv[1]))
time.sleep(3)
flag = p.read32(0x004e2778) + 0x5c
p.write32(flag, 1)
time.sleep(0.5)
print('frame', p.read32(0x00479d04), 'cheat flag at', hex(flag), '=', p.read32(flag), flush=True)
p.save_state(int(sys.argv[2]))
time.sleep(3)
print('saved slot', sys.argv[2], flush=True)
