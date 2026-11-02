"""C14: write a PCSX2 input recording (.p2m2) from a list of held-input segments, for a pad-less capture.

usage: make_tape.py TEMPLATE.p2m2 OUT.p2m2 SEGMENT...

Each SEGMENT is FIELDS[:BUTTONS[:LX,LY]]: hold BUTTONS (names joined by '+', or '-' for none) and the
left stick (0..255, 127 centred, y 0 up) for FIELDS fields. The header's version, emulator, author and
game come from TEMPLATE, as P21's make_p2m2.py does; from_savestate is set, so OUT needs a
OUT_SaveState.p2s beside it. Port 0's 18 bytes: the two button bytes (active low), RX RY LX LY, then the
twelve pressures R L U D Tri Cir X Sq L1 R1 L2 R2.
"""
import sys

BYTE0 = {'select': 0x01, 'l3': 0x02, 'r3': 0x04, 'start': 0x08,
         'up': 0x10, 'right': 0x20, 'down': 0x40, 'left': 0x80}
BYTE1 = {'l2': 0x01, 'r2': 0x02, 'l1': 0x04, 'r1': 0x08,
         'triangle': 0x10, 'circle': 0x20, 'cross': 0x40, 'square': 0x80}
PRESSURE = ['right', 'left', 'up', 'down', 'triangle', 'circle', 'cross', 'square', 'l1', 'r1', 'l2', 'r2']


def field(buttons, lx=127, ly=127):
    b0 = b1 = 0xff
    press = [0] * 12
    for name in buttons:
        if name in BYTE0:
            b0 &= ~BYTE0[name]
        else:
            b1 &= ~BYTE1[name]
        if name in PRESSURE:
            press[PRESSURE.index(name)] = 0xff
    return bytes([b0, b1, 127, 127, lx, ly] + press)


def main():
    tpl = open(sys.argv[1], 'rb').read()
    fields = []
    for seg in sys.argv[3:]:
        parts = seg.split(':')
        n = int(parts[0])
        buttons = [] if len(parts) < 2 or parts[1] == '-' else parts[1].split('+')
        lx, ly = (127, 127) if len(parts) < 3 else map(int, parts[2].split(','))
        fields += [field(buttons, lx, ly)] * n
    n = len(fields)
    head = bytearray(tpl[:561]) + n.to_bytes(4, 'little') + (0).to_bytes(4, 'little') + bytes([1])
    assert len(head) == 570
    out = bytes(head) + bytes(36) + b''.join(f + bytes(18) for f in fields)
    open(sys.argv[2], 'wb').write(out)
    print(sys.argv[2], n, 'fields')


main()
