"""Write a PCSX2 input recording that holds Cross (full pressure) with both sticks centred.
Usage: make_p2m2.py TEMPLATE.p2m2 OUT.p2m2 FIELDS
The header's version, emulator, author and game come from TEMPLATE; from_savestate is set."""
import sys
tpl = open(sys.argv[1], 'rb').read()
n = int(sys.argv[3])
head = bytearray(tpl[:561])
head += n.to_bytes(4, 'little') + (0).to_bytes(4, 'little') + bytes([1])
assert len(head) == 570
pad0 = bytes([0xff, 0xbf, 0x7f, 0x7f, 0x7f, 0x7f] + [0] * 6 + [0xff] + [0] * 5)
assert len(pad0) == 18
out = bytes(head) + bytes(36) + (pad0 + bytes(18)) * n
open(sys.argv[2], 'wb').write(out)
print(sys.argv[2], len(out), 'bytes,', n, 'fields')
