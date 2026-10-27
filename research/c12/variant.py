"""C12: write a variant of the 2p tape, to test what parts the playback from the user's ride.

    python3 -B research/c12/variant.py KIND OUT.p2m2

KIND:
  same      the tape as it is (a control)
  zero0     field 0 (all zero bytes: every button held, sticks hard up-left) made neutral
  drop1     the whole tape a field earlier (field i plays field i+1)
  delay1    the whole tape a field later (a neutral field first)
The header's frame count follows the field count. Research, not product.
"""
import os
import struct
import sys

HEAD = 570
NEUTRAL = bytes([0xff, 0xff, 0x7f, 0x7f, 0x7f, 0x7f]) + bytes(12)
raw = open(os.path.join('context', 'recordings', 'split', '2p.p2m2'), 'rb').read()
head, body = bytearray(raw[:HEAD]), raw[HEAD:]
fields = [body[36 * i:36 * i + 36] for i in range(len(body) // 36)]
kind, out = sys.argv[1], sys.argv[2]
if kind == 'zero0':
    fields[0] = NEUTRAL * 2
elif kind == 'drop1':
    fields = fields[1:]
elif kind == 'delay1':
    fields = [NEUTRAL * 2] + fields
elif kind != 'same':
    sys.exit('unknown kind ' + kind)
frames = struct.unpack_from('<I', head, 561)[0] + len(fields) - len(body) // 36
struct.pack_into('<I', head, 561, frames)
open(out, 'wb').write(bytes(head) + b''.join(fields))
print(kind, len(fields), 'fields, header frames', frames)
