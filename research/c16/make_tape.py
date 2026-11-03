"""C16: write a two-pad PCSX2 input recording (.p2m2) from held-input segments, for a pad-less capture.
C14's research/c14/make_tape.py with port 1 written idle (C14's zero bytes hold every button down on
port 1, which a two-player state reads) and a right stick.

usage: make_tape.py TEMPLATE.p2m2 OUT.p2m2 SEGMENT...

Each SEGMENT is FIELDS[:BUTTONS[:LX,LY[:RX,RY]]]: hold BUTTONS (names joined by '+', or '-' for none)
and the sticks (0..255, 127 centred, y 0 up) for FIELDS fields. A BUTTON may carry its pressure as
name=0..255 (default 255). The header comes from TEMPLATE with the field count set and from_savestate
on, so OUT needs an OUT_SaveState.p2s beside it. The 2p state reads one field a tick (60 Hz, C10).
"""
import sys

BYTE0 = {'select': 0x01, 'l3': 0x02, 'r3': 0x04, 'start': 0x08,
         'up': 0x10, 'right': 0x20, 'down': 0x40, 'left': 0x80}
BYTE1 = {'l2': 0x01, 'r2': 0x02, 'l1': 0x04, 'r1': 0x08,
         'triangle': 0x10, 'circle': 0x20, 'cross': 0x40, 'square': 0x80}
PRESSURE = ['right', 'left', 'up', 'down', 'triangle', 'circle', 'cross', 'square', 'l1', 'r1', 'l2', 'r2']
IDLE = bytes([0xff, 0xff, 0x7f, 0x7f, 0x7f, 0x7f] + [0] * 12)


def field(buttons, lx=127, ly=127, rx=127, ry=127):
    b0 = b1 = 0xff
    press = [0] * 12
    for b in buttons:
        name, _, level = b.partition('=')
        if name in BYTE0:
            b0 &= ~BYTE0[name]
        else:
            b1 &= ~BYTE1[name]
        if name in PRESSURE:
            press[PRESSURE.index(name)] = int(level) if level else 0xff
    return bytes([b0, b1, rx, ry, lx, ly] + press)


def fields(segments):
    out = []
    for seg in segments:
        parts = seg.split(':')
        n = int(parts[0])
        buttons = [] if len(parts) < 2 or parts[1] == '-' else parts[1].split('+')
        lx, ly = (127, 127) if len(parts) < 3 or parts[2] == '-' else map(int, parts[2].split(','))
        rx, ry = (127, 127) if len(parts) < 4 else map(int, parts[3].split(','))
        out += [field(buttons, lx, ly, rx, ry)] * n
    return out


def main():
    tpl = open(sys.argv[1], 'rb').read()
    p1 = fields(sys.argv[3:])
    n = len(p1)
    head = bytearray(tpl[:561]) + n.to_bytes(4, 'little') + (0).to_bytes(4, 'little') + bytes([1])
    assert len(head) == 570
    out = bytes(head) + bytes(36) + b''.join(f + IDLE for f in p1)
    open(sys.argv[2], 'wb').write(out)
    print(sys.argv[2], n, 'fields')


if __name__ == '__main__':
    main()
