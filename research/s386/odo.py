"""S386: record each bike's statistics odometer and what its writer reads, every tick of an sx01 race.

Run straight after research/c31/boot.py (4 racers, all ridden by the AI) in the same `./kit pcsx2 with`
job. The odometer (bike +0x133c) is added to at the top of the player bike's update, before its
physics: the forward speed, front wheel touching (+0x7b0 -> +0xdc) the bike's speed (+0x438), else
the chassis' velocity (+0x7a4 -> +0x260) along the transform's flattened forward row, never below 0,
times dt. Each row keeps, per bike (by card id): the bike's +0x430 (16 bytes), +0x748 (the wreck
reason), +0x1330 (16 bytes, the odometer at +0xc), the front wheel's +0xdc, the chassis' +0x260 (16
bytes) and the transform's rows +0x80..+0xc0. Also the frame, dt and the score manager's 0x110 bytes
(its clock at +0xf8).

usage: python3 research/s386/odo.py OUT.jsonl [TICKS]
"""
import json, sys, time
sys.path.insert(0, '.')
from ps2kit.pine import Pine

out_path = sys.argv[1]
ticks = int(sys.argv[2]) if len(sys.argv) > 2 else 1500
ACTIVITY_MANAGER, FRAME, DT, IN_GAME = 0x004e1df8, 0x00479d04, 0x00479cd4, 0x004e2650

p = Pine(check=False, lock=False, timeout=10.0)
while p.read32(IN_GAME) != 1:
    time.sleep(0.1)
act = 0
while not act:
    m = p.read32(ACTIVITY_MANAGER)
    lst = p.read32(m + 0x58) if m else 0
    act = p.read32(lst) if lst else 0
    time.sleep(0.05)
mgr = act + 0x50
cards = {}
while len(cards) < 4:
    c = p.read32(mgr + 0x34)
    while c:
        cards[p.read32(c)] = c
        c = p.read32(c + 0x1c8)
    time.sleep(0.05)
cards = [cards[k] for k in range(4)]
# The card's owner is the bike's listener at +0xf9c, whose first word is the card.
bikes = [p.read32(c + 4) - 0xf9c for c in cards]
for c, b in zip(cards, bikes):
    assert p.read32(b + 0xf9c) == c, 'bike %#x does not hold card %#x' % (b, c)
chassis = [p.read32(b + 0x7a4) for b in bikes]
front = [p.read32(b + 0x7b0) for b in bikes]
node = [p.read32(c + 0x254) for c in chassis]
print('manager %#x cards %s bikes %s' % (mgr, [hex(c) for c in cards], [hex(b) for b in bikes]), flush=True)

parts = [('bike430', 0x10), ('reason', 4), ('bike1330', 0x10), ('front', 4), ('vel', 0x10), ('rows', 0x40)]
regions = [(FRAME, 4), (DT, 4), (mgr, 0x110)]
for k in range(4):
    regions += [(bikes[k] + 0x430, 0x10), (bikes[k] + 0x748, 4), (bikes[k] + 0x1330, 0x10), (front[k] + 0xdc, 4),
                (chassis[k] + 0x260, 0x10), (node[k] + 0x80, 0x40)]
out = open(out_path, 'w')
last, rows, skipped = p.read32(FRAME), 0, 0
while rows < ticks:
    f = p.next_frame(last, addr=FRAME)
    last = f
    b = p.settle(regions, f)
    if b is None:
        skipped += 1
        continue
    row = {'frame': int.from_bytes(b[0:4], 'little'), 'dt': b[4:8].hex(), 'mgr': b[8:8 + 0x110].hex()}
    at = 8 + 0x110
    for k in range(4):
        for name, n in parts:
            row['%s%d' % (name, k)] = b[at:at + n].hex()
            at += n
    out.write(json.dumps(row) + '\n')
    rows += 1
out.close()
print('rows', rows, 'skipped', skipped, flush=True)
