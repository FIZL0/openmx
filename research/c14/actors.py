"""C14: list the game manager's actors (node +0 actor, +8 next): type, class vtable, player, ctl source.
usage: actors.py  (KIT_PCSX2 set; the heli's MX vtable word is 0x0045c980, its +0x364 the flown flag)"""
import sys
sys.path.insert(0, '.')
from ps2kit.pine import Pine
p = Pine()
gm = p.read32(0x004e1e98)
node = p.read32(p.read32(gm + 0x34) + 0x34) if gm else 0
i = 0
while node and i < 40:
    a = p.read32(node)
    pl = p.read32(a + 0x20) if a else 0
    print(i, hex(node), hex(a), 'type', p.read32(a), 'vt', hex(p.read32(a + 0x48)), 'player', hex(pl),
          'src', p.read32(pl + 0x34) if pl else '-', 'heli_flown' if p.read32(a + 0x48) == 0x0045c980 and p.read32(a + 0x364) else '')
    node = p.read32(node + 8)
    i += 1
print('frame', p.frame())
