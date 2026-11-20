"""R22: a two-pad PCSX2 input recording (.p2m2, from a save state) from two pad scripts, timed in game ticks.

  python3 -I research/r22/tape2.py TEMPLATE.p2m2 P1.pad P2.pad OUT.p2m2

C15's tape.py (research/c15/tape.py: the same script language, `down/up <btn>`, `stick l|r x y`, `release`,
`sleep <N>t`) with port 1 filled from the second script. TEMPLATE gives the header (version, emulator, author,
game name): the split tape's own (context/recordings/split/splitscreen2_shortcolision.p2m2). Each tick is
two fields, as the user's split tapes play. The shorter script holds its last pad to the end.
Also writes OUT.ticks.csv: both pads per tick. Research, not product.
"""
import csv
import sys

HEAD = 570
FIELDS_PER_TICK = 2
BUTTONS = {
    'select': (0, 0x01, None), 'l3': (0, 0x02, None), 'r3': (0, 0x04, None), 'start': (0, 0x08, None),
    'up': (0, 0x10, 2), 'right': (0, 0x20, 0), 'down': (0, 0x40, 3), 'left': (0, 0x80, 1),
    'l2': (1, 0x01, 10), 'r2': (1, 0x02, 11), 'l1': (1, 0x04, 8), 'r1': (1, 0x08, 9),
    'triangle': (1, 0x10, 4), 'circle': (1, 0x20, 5), 'cross': (1, 0x40, 6), 'square': (1, 0x80, 7),
}


def axis(v):
    v = max(-1.0, min(1.0, float(v)))
    return 0x7f if v == 0 else round(0x7f + v * (0x80 if v > 0 else 0x7f))


def ticks(path):
    """The script as one (held, lx, ly, rx, ry) per tick."""
    held, stick, out = set(), {'l': (0.0, 0.0), 'r': (0.0, 0.0)}, []
    for line in open(path):
        w = line.split('#')[0].split()
        if not w:
            continue
        if w[0] in ('down', 'up'):
            held.discard(next((h for h in held if h.split('@')[0] == w[1]), None))
            if w[0] == 'down':
                held.add(f'{w[1]}@{round(float(w[2]) * 0xff)}' if len(w) > 2 else w[1])
        elif w[0] == 'stick':
            stick[w[1]] = (float(w[2]), float(w[3]))
        elif w[0] == 'release':
            held.clear()
            stick = {'l': (0.0, 0.0), 'r': (0.0, 0.0)}
        elif w[0] == 'sleep':
            n = int(w[1][:-1]) if w[1].endswith('t') else round(int(w[1]) * 30 / 1000)
            out += [(frozenset(held), *stick['l'], *stick['r'])] * n
        else:
            raise SystemExit(f'{path}: unknown command {line.strip()!r}')
        for b in held:
            assert b.split('@')[0] in BUTTONS, f'unknown button {b}'
    return out


def pad(held, lx, ly, rx, ry):
    f = bytearray([0xff, 0xff, axis(rx), axis(ry), axis(lx), axis(ly)] + [0] * 12)
    for b in held:
        name, _, level = b.partition('@')
        byte, bit, p = BUTTONS[name]
        f[byte] &= ~bit & 0xff
        if p is not None:
            f[6 + p] = int(level) if level else 0xff
    return bytes(f)


def main():
    template, p1, p2, out = sys.argv[1:5]
    head = bytearray(open(template, 'rb').read(HEAD))
    a, b = ticks(p1), ticks(p2)
    idle = (frozenset(), 0.0, 0.0, 0.0, 0.0)
    n = max(len(a), len(b))
    a += [a[-1] if a else idle] * (n - len(a))
    b += [b[-1] if b else idle] * (n - len(b))
    body = [bytes(36)]
    for x, y in zip(a, b):
        body += [pad(*x) + pad(*y)] * FIELDS_PER_TICK
    head[561:565] = (len(body) - 1).to_bytes(4, 'little')
    head[569] = 1
    open(out, 'wb').write(bytes(head) + b''.join(body))
    with open(out + '.ticks.csv', 'w', newline='') as fh:
        w = csv.writer(fh)
        w.writerow(['tick', 'p1', 'p1_lx', 'p1_ly', 'p2', 'p2_lx', 'p2_ly'])
        for i, (x, y) in enumerate(zip(a, b)):
            w.writerow([i, ' '.join(sorted(x[0])), x[1], x[2], ' '.join(sorted(y[0])), y[1], y[2]])
    print(out, len(body) - 1, 'fields,', n, 'ticks')


main()
