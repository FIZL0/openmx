"""C33: list a free09 RAM image's riders: actor-list order, rider, body (rider +0x7a4), and for an AI
rider its interface and brain.

The actor list is @game_manager +0x34 -> +0x34, nodes (actor +0, next +8). Every rider, the human's
too, has an interface object holding the vtable 0x004550e4 at +8 and the rider at +0xc; an AI rider's
interface (brain +0x40) holds its brain at +0, the human's holds 0. So a scan for that vtable followed
by an actor finds the riders and their brains. (The actor's first word is no type: it was 2 on
recording2's riders and is 1 on C33's.)

usage: riders.py IMAGE   (an eeMemory.bin, or a .p2s: its eeMemory.bin is read out of the zip)
Prints one line per rider; with --json, a JSON list instead.
"""
import json, struct, sys, zipfile

GAME_MANAGER, IFACE_VT = 0x004e1e98, 0x004550e4


def ram(path):
    if zipfile.is_zipfile(path):
        with zipfile.ZipFile(path) as z:
            return z.read('eeMemory.bin')
    with open(path, 'rb') as f:
        return f.read()


def riders(d):
    u = lambda a: struct.unpack_from('<I', d, a & 0x1ffffff)[0]
    out = []
    n = u(u(u(GAME_MANAGER) + 0x34) + 0x34)
    while n and len(out) < 64:
        a = u(n)
        out.append({'rider': a, 'body': u(a + 0x7a4), 'iface': 0, 'brain': 0})
        n = u(n + 8)
    by = {r['rider']: r for r in out}
    for off in range(0, len(d) - 8, 4):
        if struct.unpack_from('<I', d, off)[0] == IFACE_VT:
            r = struct.unpack_from('<I', d, off + 4)[0]
            if r in by:
                by[r]['iface'] = off - 8
                by[r]['brain'] = u(off - 8)
    return [r for r in out if r['iface']]


if __name__ == '__main__':
    rs = riders(ram(sys.argv[1]))
    if '--json' in sys.argv:
        print(json.dumps(rs))
    else:
        for k, r in enumerate(rs):
            print(k, 'rider %#010x body %#010x iface %#010x brain %#010x %s'
                  % (r['rider'], r['body'], r['iface'], r['brain'], 'AI' if r['brain'] else 'human'))
