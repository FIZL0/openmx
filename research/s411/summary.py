"""S411: one line per row of a card.toml capture: the bike's ground and wheel loads, its stunt ids, the
backflip's and the 360's clips, the bank clock, the node rows' heights and the score card.

  python3 -I research/s411/summary.py context/capture/s411/NAME.csv [FIRST LAST]
"""
import csv
import struct
import sys

POOL = 0x012ad6c0
csv.field_size_limit(1 << 26)


def rows(path):
    for r in csv.DictReader(open(path)):
        obj = bytes.fromhex(r['obj0']) + bytes.fromhex(r['obj1'])
        node, card, pool = (bytes.fromhex(r[k]) for k in ('node', 'card', 'pool'))
        u = lambda b, o: struct.unpack_from('<I', b, o)[0]
        f = lambda b, o: struct.unpack_from('<f', b, o)[0]
        nodes, n = [], u(card, 0xc8)
        while n and len(nodes) < 100:
            k = n - POOL
            nodes.append(dict(flags=u(pool, k), id=u(pool, k + 4), points=f(pool, k + 8), held=f(pool, k + 0x1c)))
            n = u(pool, k + 0x24)
        yield dict(
            vsync=int(r['vsync']), ground=u(obj, 0x324), loads=(f(obj, 0x704), f(obj, 0x708)),
            stunt=u(obj, 0x1524), next=u(obj, 0x1528), pose=u(obj, 0x152c), clip=u(obj, 0x1754),
            spin=u(obj, 0x1758), bank=f(obj, 0x14f4), up=f(node, 0x84), fwd=f(node, 0x94),
            wrecks=u(obj, 0x6e4), trick=u(obj, 0x14ec) & 0xff, flags=u(card, 8), total=f(card, 0xdc),
            combo=f(card, 0xe0), nodes=nodes, speed=f(obj, 0xc38), flip=[u(obj, 0x1194 + 4 * k) for k in range(4)],
        )


if __name__ == '__main__':
    a, b = (int(sys.argv[2]), int(sys.argv[3])) if len(sys.argv) > 3 else (0, 10 ** 9)
    for i, r in enumerate(rows(sys.argv[1])):
        if a <= i <= b:
            ns = ' '.join('%d:%x:%.0f' % (n['id'], n['flags'], n['points']) for n in r['nodes'])
            print(i, r['vsync'], 'g%d' % r['ground'], 'ld %.0f %.0f' % r['loads'], 'v%.1f' % r['speed'],
                  'st %d>%d' % (r['stunt'], r['next']), 'clip%d spin%d' % (r['clip'], r['spin']),
                  'bank%.3f' % r['bank'], 'up%.2f fw%.2f' % (r['up'], r['fwd']), 'flip', ''.join(map(str, r['flip'])),
                  'wr%d t%d' % (r['wrecks'], r['trick']), 'card %x %.0f %.0f' % (r['flags'], r['total'], r['combo']), ns)
