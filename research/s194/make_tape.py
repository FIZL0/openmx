"""S194: a PCSX2 input recording that holds the same buttons on every field, from a kit copy's save slot.

usage: python3 -B research/s194/make_tape.py TEMPLATE.p2m2 SLOT_P2S OUT.p2m2 FIELDS BUTTONS

P21's make_p2m2.py (research/journal/2026-10-20-p21-p-a-top-speed-of-its-own/capture) for any held
buttons: the header's version, emulator, author and game come from TEMPLATE (an MX Unleashed movie),
from_savestate is set, and SLOT_P2S is copied next to OUT as its `_SaveState.p2s`. BUTTONS is a
comma list of square, cross, r2 (what S129's pads hold: `square,r2` braked, `cross` across), each at
full pressure, both sticks centred. With no /dev/uinput the kit pad cannot hold them, and a movie can.
"""

import shutil
import sys

# The movie's 18-byte pad: the button word (active low), RX RY LX LY, then the pressures in the order
# right left up down triangle circle cross square l1 r1 l2 r2.
BUTTONS = {'cross': (1, 0x40, 12), 'square': (1, 0x80, 13), 'r2': (1, 0x02, 17)}

tpl, state, out, n = sys.argv[1], sys.argv[2], sys.argv[3], int(sys.argv[4])
pad = bytearray([0xff, 0xff, 0x7f, 0x7f, 0x7f, 0x7f] + [0] * 12)
for b in sys.argv[5].split(','):
    byte, bit, pressure = BUTTONS[b]
    pad[byte] &= ~bit & 0xff
    pad[pressure] = 0xff
head = bytearray(open(tpl, 'rb').read()[:561])
head += n.to_bytes(4, 'little') + (0).to_bytes(4, 'little') + bytes([1])
assert len(head) == 570
open(out, 'wb').write(bytes(head) + bytes(36) + (bytes(pad) + bytes(18)) * n)
shutil.copyfile(state, out + '_SaveState.p2s')
print(out, n, 'fields, pad', pad[:2].hex())
