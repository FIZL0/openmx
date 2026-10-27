"""Which bike and rider each 2p player has (C10): the strings each rider's actors point at.

    python3 -B research/c10/looks.py c10/2p-start        # context/states/<name>.ram

For P1 and P2 (vehicle, bike actor, rider body = bike - 0x400), every word in the first 0xc00 bytes that
points at a printable string, and one level further (a pointer to a struct whose first words point at
strings), printed with its offset. Research, not product.
"""
import os
import struct
import sys

GAME_MANAGER = 0x004e1e98
ram = open(os.path.join('context', 'states', sys.argv[1] + '.ram'), 'rb').read()


def u32(a):
    return struct.unpack_from('<I', ram, a & 0x1ffffff)[0]


def cstr(a, n=64):
    if not 0x100000 <= a < 0x2000000:
        return None
    b = ram[a:a + n].split(b'\0')[0]
    return b.decode('latin1') if len(b) >= 4 and all(32 <= c < 127 for c in b) else None


node = u32(u32(u32(GAME_MANAGER) + 0x34) + 0x34)
items = []
while node and len(items) < 16:
    items.append(u32(node))
    node = u32(node + 8)
who = {'P1': (items[3], items[4]), 'P2': (items[2], items[5])}
for p, (veh, bike) in who.items():
    for lab, base in (('vehicle', veh), ('bike', bike), ('body', bike - 0x400)):
        seen = set()
        for off in range(0, 0xc00, 4):
            w = u32(base + off)
            s = cstr(w)
            if s:
                print('%s %s +0x%x -> %r' % (p, lab, off, s))
                continue
            if 0x100000 <= w < 0x2000000:
                for k in range(0, 0x40, 4):
                    s = cstr(u32(w + k))
                    if s and s not in seen:
                        seen.add(s)
                        print('%s %s +0x%x -> +0x%x -> %r' % (p, lab, off, k, s))
