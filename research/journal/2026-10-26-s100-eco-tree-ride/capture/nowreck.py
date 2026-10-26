"""S100: load the tree state (scratch slot 9), clear the original's wrecks-enabled word (oracle.txt
wrecks_enabled) while paused, and save it to scratch slot 8. Usage: nowreck.py PID"""
import sys, time
sys.path.insert(0, '.')
sys.path.insert(0, 'research/journal/2026-10-20-c6-c6-recording2-capture/playback')
from ps2kit.pine import Pine
import atspi_lib as A
from gi.repository import Atspi

WRECKS_ENABLED = 0x00420bc0
app = A.app_of(int(sys.argv[1]))
item = A.menu_path(app, ['System', 'Pause'])
def paused():
    return item.get_state_set().contains(Atspi.StateType.CHECKED)

p = Pine()
if not paused():
    A.press(item)
time.sleep(0.5)
p.load_state(9)
time.sleep(3)
if not paused():
    A.press(item)
time.sleep(0.5)
f0 = p.read32(0x479d04); time.sleep(0.3)
assert p.read32(0x479d04) == f0, 'not paused'
print('frame', f0, 'wrecks_enabled was', p.read32(WRECKS_ENABLED))
p.write32(WRECKS_ENABLED, 0)
p.save_state(8)
time.sleep(2)
print('saved 8 at frame', p.read32(0x479d04), 'wrecks_enabled', p.read32(WRECKS_ENABLED))
