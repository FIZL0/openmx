"""C15: write a PCSX2 input recording (.p2m2, from a save state) from a pad script, timed in game ticks.

  python3 -I research/c15/tape.py TEMPLATE.p2m2 SCRIPT.pad OUT.p2m2 [--game NAME]

TEMPLATE gives the header's version, emulator and author (context/recordings/animandpump.p2m2); the game
name is rewritten. SCRIPT is ./kit pad's language: `down/up <btn>`, `stick l|r <x> <y>` (-1..1, y -1 up),
`release`, `sleep <N>t` (N game ticks) or `sleep <ms>` (rounded to ticks), `# comment`. Each tick is two
fields: the tapes this project made play two fields a game tick in PCSX2 (S100, C8). Pad 2 is blank.

Also writes OUT.ticks.csv: the pad held on every tick (tick 0 is the first field pair after the state),
which is what a replay feeds the port.
"""
import argparse
import csv

HEAD = 570                      # version, emulator[50], author[255], game[255], fields u32, undo u32, from-state u8
GAME = slice(1 + 50 + 255, 1 + 50 + 255 + 255)
FIELDS_PER_TICK = 2
# byte 0 / byte 1 bits (active low) and pressure index (R L U D Tri Cir X Sq L1 R1 L2 R2 from byte 6)
BUTTONS = {
    'select': (0, 0x01, None), 'l3': (0, 0x02, None), 'r3': (0, 0x04, None), 'start': (0, 0x08, None),
    'up': (0, 0x10, 2), 'right': (0, 0x20, 0), 'down': (0, 0x40, 3), 'left': (0, 0x80, 1),
    'l2': (1, 0x01, 10), 'r2': (1, 0x02, 11), 'l1': (1, 0x04, 8), 'r1': (1, 0x08, 9),
    'triangle': (1, 0x10, 4), 'circle': (1, 0x20, 5), 'cross': (1, 0x40, 6), 'square': (1, 0x80, 7),
}


def axis(v):
    """-1..1 to the pad's byte: 0 full left/up, 0x7f centre, 0xff full right/down."""
    v = max(-1.0, min(1.0, float(v)))
    return 0x7f if v == 0 else round(0x7f + v * (0x80 if v > 0 else 0x7f))


def parse(path):
    """The script as (ticks, held buttons, lx, ly, rx, ry) spans."""
    held, stick = set(), {'l': (0.0, 0.0), 'r': (0.0, 0.0)}
    spans = []
    for line in open(path):
        w = line.split('#')[0].split()
        if not w:
            continue
        if w[0] == 'down':
            held.add(w[1])
        elif w[0] == 'up':
            held.discard(w[1])
        elif w[0] == 'stick':
            stick[w[1]] = (float(w[2]), float(w[3]))
        elif w[0] == 'release':
            held.clear()
            stick = {'l': (0.0, 0.0), 'r': (0.0, 0.0)}
        elif w[0] == 'sleep':
            n = int(w[1][:-1]) if w[1].endswith('t') else round(int(w[1]) * 30 / 1000)
            spans.append((n, frozenset(held), *stick['l'], *stick['r']))
        else:
            raise SystemExit(f'{path}: unknown command {line.strip()!r}')
        for b in held:
            assert b in BUTTONS, f'unknown button {b}'
    return spans


def field(held, lx, ly, rx, ry):
    f = bytearray([0xff, 0xff, axis(rx), axis(ry), axis(lx), axis(ly)] + [0] * 12)
    for b in held:
        byte, bit, p = BUTTONS[b]
        f[byte] &= ~bit & 0xff
        if p is not None:
            f[6 + p] = 0xff
    return bytes(f) + bytes(18)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('template')
    ap.add_argument('script')
    ap.add_argument('out')
    ap.add_argument('--game', default='MX vs. ATV Unleashed')
    a = ap.parse_args()
    head = bytearray(open(a.template, 'rb').read(HEAD))
    head[GAME] = a.game.encode().ljust(255, b'\0')
    body, ticks = [bytes(36)], []          # field 0 is the blank one before the first (as PCSX2 writes)
    for n, held, lx, ly, rx, ry in parse(a.script):
        f = field(held, lx, ly, rx, ry)
        for _ in range(n):
            body += [f] * FIELDS_PER_TICK
            ticks.append((' '.join(sorted(held)), lx, ly, rx, ry))
    head[561:565] = (len(body) - 1).to_bytes(4, 'little')
    head[569] = 1                          # from a save state
    open(a.out, 'wb').write(bytes(head) + b''.join(body))
    with open(a.out + '.ticks.csv', 'w', newline='') as fh:
        w = csv.writer(fh)
        w.writerow(['tick', 'buttons', 'lx', 'ly', 'rx', 'ry'])
        for i, t in enumerate(ticks):
            w.writerow([i, *t])
    print(a.out, len(body) - 1, 'fields,', len(ticks), 'ticks')


main()
