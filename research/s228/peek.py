"""S228: the frame, dt, track and player records of an EE RAM image.
usage: python3 -B research/s228/peek.py RAM.bin PLAYER_ADDR..."""
import struct, sys
ram = open(sys.argv[1], 'rb').read()
r = lambda a, f='<I': struct.unpack_from(f, ram, a & 0x1ffffff)[0]
print('frame', r(0x479d04), 'dt', r(0x479cd4, '<f'), 'track', ram[0x479d9c:0x479d9c + 0x40].split(b'\0')[0])
for pl in sys.argv[2:]:
    pl = int(pl, 16)
    print(hex(pl), 'src', r(pl + 0x34), 'screen', r(pl + 0x50))
