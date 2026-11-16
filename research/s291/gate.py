"""S291: a live sx01 start with riders teleported through the closed start gate, every tick recorded.

usage: python3 -B research/s291/gate.py PID OUT.csv AT IDS [AFTER [ORDER]]

Loads C31's start state (context/states/c31-sx01-race-start.p2s, copied over the copy's slot 8), lets the
countdown run until its elapsed time reaches AT seconds, pauses the VM over AT-SPI (PCSX2 `PID`'s
System > Pause), moves the bikes of the cards whose ids are in IDS (comma list, 0 the human) three feet
through the closed gate along its normal (every triple near the bike's position in the bike, its
chassis and its frame object, as research/s134_tele.py does), saves the paused state into slot 9 and
resumes. With ORDER (card ids, '/'-joined) the card list is relinked in that order first. It records
one row per race manager tick from the cards' first tick to AFTER ticks (default 60) past the teleport,
on one PINE connection (no recorder beside it): a row is read the moment the score manager's countdown
elapsed word moves, which is the end of the race manager's tick (the bikes update before it in the
frame), so it runs out at the drop. A frame_counter trigger reads rows mid-tick at random phases.

Rows: `phase` is `run` for a tick, `pause` for the paused state before the move and `moved` for it
after. Per row: frame_counter, dt, rand_state, the gate's flags, the score manager's flags and elapsed,
the race's music (+0x4c0) and tip (+0x410) clocks, the scene's word (+0xf0), the card list's order (card
ids, '/'-joined), the countdown's length, the gate's first 0x80 bytes, and per card id k: `kK_now`/`kK_last` (card +0x60/+0x70), `kK_pen` (+0x1c0), the bike
frame's position (+0x30) and look (+0x20) rows, the chassis' velocity (+0x260) and its six freedom words
(+0x2a0). Floats are written as hex bit patterns, four per quad joined by ':'.

Run under `./kit pcsx2 with --`, PCSX2 started with QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1.
"""

import shutil
import struct
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / 'research/journal/2026-10-20-c6-c6-recording2-capture/playback'))
from ps2kit.config import cfg            # noqa: E402
from ps2kit.pine import Pine             # noqa: E402
import atspi_lib as A                    # noqa: E402
from gi.repository import Atspi          # noqa: E402

PID = int(sys.argv[1])
OUT = Path(sys.argv[2])
AT = float(sys.argv[3])
IDS = [int(s) for s in sys.argv[4].split(',') if s]
AFTER = int(sys.argv[5]) if len(sys.argv) > 5 else 60
ORDER = [int(x) for x in sys.argv[6].split('/')] if len(sys.argv) > 6 else None
SYM = dict(cfg().symbols)
for line in open(ROOT / 'research/oracle.txt'):       # the names kit.toml does not carry
    w = line.split()
    if len(w) > 2 and w[1] == 'boot' and w[0] not in SYM:
        SYM[w[0]] = int(w[2], 16)
FRAME, DT, RAND, ACT, SCENE = (SYM[n] for n in ('frame_counter', 'frame_dt', 'rand_state', 'activity_manager',
                                                 'scene'))
STATES = Path('/home/rycs/repos/openmx-slots/pcsx2/s5/PCSX2/savestates')
SLOT = STATES / 'SLUS-20767 (9866FE54).08.p2s'

app = A.app_of(PID)
item = A.menu_path(app, ['System', 'Pause'])


def paused():
    return item.get_state_set().contains(Atspi.StateType.CHECKED)


def ok(a):
    return 0x100000 <= a < 0x2000000


shutil.copyfile(ROOT / 'context/states/c31-sx01-race-start.p2s', SLOT)
p = Pine()
if paused():
    A.press(item)
p.load_state(8)
time.sleep(1.0)


def race():
    a = p.read32(ACT)
    return p.read32(p.read32(a + 0x58)) if ok(a) and ok(p.read32(a + 0x58)) else 0


def cards(r):
    """Card addresses by id, and the list's order of ids."""
    by, order = {}, []
    c = p.read32(r + 0x84)
    while ok(c) and len(order) < 8:
        k = p.read32(c)
        by[k] = c
        order.append(k)
        c = p.read32(c + 0x1c8)
    return by, order


def bike_of(card):
    b = p.read32(card + 4) - 0xf9c
    ch = p.read32(b + 0x7a4)
    obj = p.read32(ch + 0x254) + 0x80
    return b, ch, obj


def hexq(b, off, n=4):
    return ':'.join('%08x' % w for w in struct.unpack_from('<%dI' % n, b, off))


# Wait for the race and its cards.
f = p.frame(FRAME)
while True:
    f = p.next_frame(f, addr=FRAME)
    r = race()
    if r and ok(p.read32(r + 0x3dc)) and cards(r)[0]:
        break
mgr = r + 0x50
gate = p.read32(r + 0x3dc)
scene = p.read32(SCENE)
by, order = cards(r)
ids = sorted(by)
bikes = {k: bike_of(by[k]) for k in ids}
print('race', hex(r), 'gate', hex(gate), 'cards', {k: hex(v) for k, v in by.items()}, 'order', order,
      'bikes', {k: tuple(map(hex, v)) for k, v in bikes.items()}, flush=True)
regions = [(FRAME, 4), (DT, 4), (RAND, 8), (gate, 0x80), (mgr, 0xd0), (r + 0x410, 4), (r + 0x4c0, 4),
           (scene + 0xf0, 4)]
for k in ids:
    _, ch, obj = bikes[k]
    regions += [(by[k], 0x1d0), (obj + 0x20, 0x20), (ch + 0x260, 0x10), (ch + 0x2a0, 0x18)]
head = ['phase', 'frame', 'dt', 'rng', 'gate_flags', 'mgr_flags', 'elapsed', 'tip', 'music', 'scene', 'order', 'length', 'gate']
for k in ids:
    head += ['k%d_%s' % (k, n) for n in ('now', 'last', 'pen', 'pos', 'look', 'vel', 'free')]


def row(phase):
    b = p.read_regions(regions)
    o = 0
    parts = []
    for _, n in regions:
        parts.append(b[o:o + n])
        o += n
    fr, dt, rng, g, m, tip, music, sc = parts[:8]
    _, order = cards(r)
    out = [phase, str(struct.unpack('<I', fr)[0]), hexq(dt, 0, 1), '%016x' % struct.unpack('<Q', rng)[0],
           hexq(g, 0x30, 1), hexq(m, 0x5c, 1), hexq(m, 0xc4, 1), hexq(tip, 0, 1), hexq(music, 0, 1),
           hexq(sc, 0, 1), '/'.join(map(str, order)), hexq(m, 0xbc, 1), hexq(g, 0, 0x20)]
    for i, k in enumerate(ids):
        c, look, vel, free = parts[8 + 4 * i: 12 + 4 * i]
        out += [hexq(c, 0x60), hexq(c, 0x70), hexq(c, 0x1c0, 1), hexq(look, 0x10), hexq(look, 0), hexq(vel, 0),
                hexq(free, 0, 6)]
    return out


def f32(h):
    return struct.unpack('<f', struct.pack('<I', int(h, 16)))[0]


def move(k):
    """Every triple near bike k's position, three feet past the gate along its normal."""
    b, ch, obj = bikes[k]
    pos = struct.unpack('<3f', p.read_block(obj + 0x30, 12))
    centre = struct.unpack('<3f', p.read_block(gate + 0x40, 12))
    normal = struct.unpack('<3f', p.read_block(gate + 0x50, 12))
    along = sum((pos[i] - centre[i]) * normal[i] for i in range(3))
    d = [normal[i] * (3.0 - along) for i in range(3)]
    d[1] = 0.0
    moved = 0
    for base, n in [(b, 0x2000), (ch, 0x400), (obj - 0x80, 0x100)]:
        blk = p.read_block(base, n)
        off = 0
        while off + 12 <= n:
            w = struct.unpack_from('<3f', blk, off)
            if all(abs(w[i] - pos[i]) < 0.5 for i in range(3)):
                for i in range(3):
                    p.write_f32(base + off + 4 * i, w[i] + d[i])
                moved += 1
                off += 12
            else:
                off += 4
    print('bike', k, 'at', pos, 'was', round(along, 2), 'ft along the normal; moved', moved, 'triples by', d,
          flush=True)


def tick(prev):
    """Wait for the countdown's elapsed word to move off `prev` (the race manager's tick has just run);
    None if it has not moved for a second (the countdown is over)."""
    end = time.time() + 1.0
    while time.time() < end:
        w = p.read32(mgr + 0xc4)
        if w != prev:
            return w
    return None


def relink(want):
    """Put the card list in the order of ids `want` (manager +0x34, card +0x1c8)."""
    p.write32(r + 0x84, by[want[0]])
    for a, b in zip(want, want[1:]):
        p.write32(by[a] + 0x1c8, by[b])
    p.write32(by[want[-1]] + 0x1c8, 0)


rows = []
left = None
el = p.read32(mgr + 0xc4)
while left is None or left > 0:
    el = tick(el)
    if el is None:
        break
    out = row('run')
    rows.append(out)
    if left is not None:
        left -= 1
    elif f32(out[6]) >= AT:
        A.press(item)
        end = time.time() + 5
        while not paused() and time.time() < end:
            time.sleep(0.05)
        time.sleep(0.5)
        f0 = p.read32(FRAME)
        time.sleep(0.3)
        assert p.read32(FRAME) == f0, 'not paused'
        rows.append(row('pause'))
        if ORDER:
            relink(ORDER)
        for k in IDS:
            move(k)
        rows.append(row('moved'))
        p.save_state(9)
        time.sleep(2.0)
        el = p.read32(mgr + 0xc4)
        A.press(item)
        left = AFTER
OUT.parent.mkdir(parents=True, exist_ok=True)
with open(OUT, 'w') as fh:
    fh.write(','.join(head) + '\n')
    for out in rows:
        fh.write(','.join(out) + '\n')
print('wrote', len(rows), 'rows to', OUT, flush=True)
