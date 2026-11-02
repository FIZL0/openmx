"""S228: list the game manager's actors from an EE RAM image (actors.py offline): type, vtable, player, ctl source.
usage: python3 -B research/s228_actors.py context/states/X.ee.bin"""
import struct, sys
ram = open(sys.argv[1], 'rb').read()
def r32(a):
    a &= 0x1ffffff
    return struct.unpack_from('<I', ram, a)[0]
def rf(a):
    return struct.unpack_from('<f', ram, a & 0x1ffffff)[0]
gm = r32(0x004e1e98)
node = r32(r32(gm + 0x34) + 0x34) if gm else 0
i = 0
while node and i < 40:
    a = r32(node)
    pl = r32(a + 0x20) if a else 0
    print(i, hex(node), hex(a), 'type', r32(a), 'vt', hex(r32(a + 0x48)), 'player', hex(pl),
          'src', r32(pl + 0x34) if pl else '-')
    node = r32(node + 8)
    i += 1
