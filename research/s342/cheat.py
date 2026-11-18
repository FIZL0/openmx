"""S342: load SLOT (boot.py's parked-baja state), set the cheat flag the pause menu's Switch To Machine
checks (S149a's switch.py), save OUT. Run inside `./kit pcsx2 with`."""
import sys, time
sys.path.insert(0, '.')
from ps2kit.pine import Pine

slot, out = int(sys.argv[1]), int(sys.argv[2])
p = Pine(check=False, lock=False, timeout=5.0)
p.load_state(slot)
time.sleep(3)
p.write32(p.read32(0x004e2778) + 0x5c, 1)
time.sleep(0.5)
p.save_state(out)
time.sleep(2)
print('cheat set, saved', out, flush=True)
