"""C18: a copy of an S149a car tape's save state with the map shortened, so the parked car is thrown.

usage: python3 -B edge_state.py IN.p2m2 OUT.p2m2 DEPTH

IN.p2m2's save state (beside it, IN.p2m2 + "_SaveState.p2s") is copied to OUT.p2m2's, and the tape too.

The car's edge test (the wreck step) asks whether the body's centre of gravity lies within 150 ft of
the map's edge, the map being (*(terrain + 0x3b4) + 0x530) x 256 x (+0x538) by (+0x534) x 256 x (+0x53c)
feet, the terrain object the car's +0x2b4 (free07: 3 x 256 x 9 = 6912 ft each way). This rewrites the
depth's count (+0x534) so the depth is DEPTH ft; with the car at z ~2357, a DEPTH of 2400 puts it past
DEPTH - 150 and the next frame throws it (reason 13) toward the shortened map's middle.
"""
import shutil
import struct
import sys
import zipfile

CAR = 0x00605270
tape, out, depth = sys.argv[1], sys.argv[2], float(sys.argv[3])
suffix = '_SaveState' + '.p2s'
src, dst = tape + suffix, out + suffix
shutil.copyfile(tape, out)
zin = zipfile.ZipFile(src)
ram = bytearray(zin.read('eeMemory' + '.bin'))
w = lambda a: struct.unpack_from('<I', ram, a)[0]
f = lambda a: struct.unpack_from('<f', ram, a)[0]
assert w(CAR + 0x48) in (0x0045cf80, 0x0045ed50), hex(w(CAR + 0x48))
area = w(w(CAR + 0x2b4) + 0x3b4)
counts = [f(area + 0x530 + 4 * i) for i in range(4)]
print('area', hex(area), 'counts/scales', counts, 'car z', f(w(CAR + 0x160) + 0x68))
struct.pack_into('<f', ram, area + 0x534, depth / (256.0 * counts[3]))
with zipfile.ZipFile(dst, 'w') as zout:
    for info in zin.infolist():
        data = bytes(ram) if info.filename == 'eeMemory' + '.bin' else zin.read(info.filename)
        zout.writestr(info, data, compress_type=info.compress_type)
print('depth now', f(area + 0x534) * 256.0 * counts[3])
