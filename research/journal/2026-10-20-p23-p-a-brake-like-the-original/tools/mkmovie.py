"""Cut a .p2m2 at field CUT and hold square (stick centred, nothing else) for HOLD more fields.

usage: mkmovie.py IN OUT CUT HOLD
"""
import struct, sys
src, out, cut, hold = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4])
m = open(src, 'rb').read()
head = bytearray(m[:570])
frames = [m[570 + 36 * k: 570 + 36 * k + 36] for k in range((len(m) - 570) // 36)]
# port 1: buttons active-low (byte 1 bit 7 = square), rx ry lx ly, 12 pressures (index 7 = square)
brake = bytes([0xff, 0x7f, 0x7f, 0x7f, 0x7f, 0x7f]) + bytes([0] * 7 + [0xff] + [0] * 4)
assert len(brake) == 18
body = frames[:cut] + [brake + frames[cut][18:]] * hold
struct.pack_into('<I', head, 561, len(body) - 1)
open(out, 'wb').write(bytes(head) + b''.join(body))
print(out, 'fields', len(body), 'cut at', cut, 'last human field', frames[cut - 1][:18].hex())
