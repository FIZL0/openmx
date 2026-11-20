"""Find Untamed's follow-camera owner (vtable 0x003ef130 at +4) in RAM images; dump its filter."""
import struct, sys

for path in sys.argv[1:]:
    data = open(path, 'rb').read()[:32 << 20]
    needle = struct.pack('<I', 0x003ef130)
    i = data.find(needle)
    while i >= 0:
        owner = i - 4
        if owner % 16 == 0 and data[owner:owner + 4] == struct.pack('<I', 1):
            f = owner + 0x40
            w = lambda o: struct.unpack_from('<4f', data, f + o)
            ring = struct.unpack_from('<I', data, f + 0x220)[0]
            print(path, hex(owner), 'target', w(0), 'eyegoal', w(0x70), 'eyepos', w(0x80),
                  'd0', w(0xd0), 'ring', ring, 'e0', w(0xe0), 'f0', w(0xf0), '200', w(0x200))
        i = data.find(needle, i + 1)
