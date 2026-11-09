"""C31: one look at the boot: emulator status, frame, settings, scene, in_game, the event's words."""
import sys
sys.path.insert(0, '.')
from ps2kit.pine import Pine

p = Pine(check=False, lock=False, timeout=5.0)
s = p.read32(0x00479d64)
print('status', p.status(), 'frame', p.frame(), 'settings %x scene %x in_game %d all_ai %d'
      % (s, p.read32(0x00479d5c), p.read32(0x004e2650), p.read32(0x004e06b8)))
if s:
    ev = p.read32(s + 0xc)
    print('racers', p.read32(s), 'event %x' % ev)
