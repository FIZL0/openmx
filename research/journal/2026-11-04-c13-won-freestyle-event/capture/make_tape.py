"""C13: a PCSX2 input recording that joins free09's stunt competition from a state in its zone.
Usage: make_tape.py TEMPLATE.p2m2 OUT.p2m2 FIELDS

Nothing held, except Circle (accept the prompt) on fields 40..79 and Cross (dismiss the
instructions; after that it is a short throttle) on fields 150..189, 250..289, 350..389 and
450..489. The header (version, emulator, author, game) comes from TEMPLATE; from_savestate is set.
The pad bytes are P21's make_p2m2.py layout: byte 1 the buttons active low (Circle 0x20,
Cross 0x40), pressures from byte 6 in the order R L U D Tri Cir X Sq."""
import sys

tpl = open(sys.argv[1], 'rb').read()
n = int(sys.argv[3])
head = bytearray(tpl[:561])
head += n.to_bytes(4, 'little') + (0).to_bytes(4, 'little') + bytes([1])
assert len(head) == 570


def pad(buttons=0xff, circle=0, cross=0):
    p = bytes([0xff, buttons, 0x7f, 0x7f, 0x7f, 0x7f] + [0] * 5 + [circle, cross] + [0] * 5)
    assert len(p) == 18
    return p + bytes(18)


idle = pad()
fields = []
for f in range(n):
    if 40 <= f < 80:
        fields.append(pad(0xdf, circle=0xff))
    elif any(s <= f < s + 40 for s in (150, 250, 350, 450)):
        fields.append(pad(0xbf, cross=0xff))
    else:
        fields.append(idle)
out = bytes(head) + bytes(36) + b''.join(fields)
open(sys.argv[2], 'wb').write(out)
print(sys.argv[2], len(out), 'bytes,', n, 'fields')
