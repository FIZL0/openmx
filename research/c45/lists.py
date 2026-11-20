"""C45: the level node's records and its dynamics world's lists in a RAM image of R63's ride (or R22's):
node 0x01a6af50's records (+0x30 count, +0x38 list), and for each list of the node world 0x01a6afc8
(+0x50 bodies, +0x68 forces, +0x80 the records resolved each pass, +0x98 pre, +0xb0 post) its members.
For each record: kind (+0), contact count (+0x100), struck words (+0x40..+0x4c), and words +0x60..+0x7c.

    python3 -B research/c45/lists.py <name under context/states, without .ram>
"""
import os, struct, sys

ram = open(os.path.join('context', 'states', sys.argv[1] + '.ram'), 'rb').read()
u32 = lambda a: struct.unpack_from('<I', ram, a & 0x1ffffff)[0]
NODE, WORLD = 0x01a6af50, 0x01a6afc8
print('frame', u32(0x00479d04))
recs = [u32(u32(NODE + 0x38) + 4 * k) for k in range(u32(NODE + 0x30))]
print('node records', ' '.join('%#x' % r for r in recs))
for off, name in ((0x50, 'bodies'), (0x68, 'forces'), (0x80, 'resolved'), (0x98, 'pre'), (0xb0, 'post')):
    n = u32(WORLD + off)
    print('world +%#x %s: %d' % (off, name, n), ' '.join('%#x' % u32(u32(WORLD + off + 8) + 4 * k) for k in range(min(n, 16))))
for r in recs:
    print('record %#x kind %d count %d struck %s +60.. %s' % (
        r, u32(r), u32(r + 0x100), [u32(r + o) for o in (0x40, 0x44, 0x48, 0x4c)],
        ' '.join('%08x' % u32(r + o) for o in range(0x60, 0x80, 4))))
