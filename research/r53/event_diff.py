"""R53: print the words of the settings struct (*0x00479d64) and its event (+0xc) that differ between
RAM images: what a front-end two-player free ride sets that a poked boot does not.

usage: python3 -B research/r53/event_diff.py A B [C ...]     (each a RAM image path without its .ram)
"""
import struct
import sys

SETTINGS = 0x00479d64
imgs = [open(p + '.ram', 'rb').read() for p in sys.argv[1:]]


def u32(m, a):
    return struct.unpack_from('<I', m, a & 0x1ffffff)[0]


def show(name, bases, n):
    print('%s at %s' % (name, ' '.join('%#x' % b for b in bases)))
    for off in range(0, n, 4):
        words = [u32(m, b + off) for m, b in zip(imgs, bases)]
        if len(set(words)) > 1:
            print('  +%#05x %s' % (off, ' '.join('%08x' % w for w in words)))


settings = [u32(m, SETTINGS) for m in imgs]
show('settings', settings, 0x200)
show('event', [u32(m, s + 0xc) for m, s in zip(imgs, settings)], 0x200)
