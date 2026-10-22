"""Find every collision record (vtable 0x00454a00) in a RAM image and print its owner-relative layout (S93): name."""
import os, struct, sys
ram = open(os.path.join('context', 'states', sys.argv[1] + '.ram'), 'rb').read()
u = lambda a: struct.unpack_from('<I', ram, a & 0x1ffffff)[0]
f = lambda a: struct.unpack_from('<f', ram, a & 0x1ffffff)[0]
pat = struct.pack('<I', 0x00454a00)
i = ram.find(pat)
while i >= 0:
    if i % 4 == 0:
        n = u(i + 0x100)
        print('%08x count %d cap %d list %08x chassis %08x body %08x hit %s rays %08x f.col %08x' % (
            i, n, u(i + 0x104), u(i + 0x108), u(i + 0xf0), u(i + 0xf4),
            [u(i + 0x40 + 4 * k) for k in range(4)], u(i + 0x1f0), u(i + 0x1f8)))
        if 0 < n <= 64:
            for k in range(n):
                e = u(i + 0x108) + k * 0x80
                print('    kind %d other %08x end_y %.3f n (%.2f %.2f %.2f) p (%.1f %.1f %.1f)' % (
                    u(e), u(e + 0x74), f(e + 0x24), f(e + 0x30), f(e + 0x34), f(e + 0x38), f(e + 0x40), f(e + 0x44), f(e + 0x48)))
    i = ram.find(pat, i + 1)
