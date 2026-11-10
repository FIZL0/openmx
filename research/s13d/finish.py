"""S13d: record an sx01 single race's finish, every tick, and end the human's race mid-way.

Run straight after research/c31/boot.py (4 racers, the human idle at the gate) in the same
`./kit pcsx2 with` job. Once AFTER AI cards have finished by the line, the human's score card (id 0) is
marked done (flags |= 1, the emulator paused for the write): the race handler's next tick finds no
player racing, the race is over, the AI still out get their finishes made up, the end is told once
(the tie draw) and the supercross activity answers the Over notice for every card — the fireworks,
the podium flashes, the Hall of Fame and the win. TAIL ticks are recorded after the poke.

Every row is one JSON line: frame, dt, rand state, the score manager's 0x110 bytes, the four cards'
0x1e0 bytes (by id), the activity's 0x4e0 bytes, the podium flashes' 0x70 bytes, the Hall of Fame
menu's 0xd0 bytes (once the activity has looked it up) and the stats block. The Hall of Fame tables
(app+0x3e0, 0x3ab4 bytes), the event and the front end's words are written once before the poke and
once at the end. The script holds the one PINE connection; nothing else may talk to the copy.

usage: python3 research/s13d/finish.py OUT.jsonl [AFTER [TAIL]]
"""
import json, sys, time
sys.path.insert(0, '.')
from ps2kit.pine import Pine

out_path = sys.argv[1]
after = int(sys.argv[2]) if len(sys.argv) > 2 else 1
tail = int(sys.argv[3]) if len(sys.argv) > 3 else 300
ACTIVITY_MANAGER, APP_POINTER, FRAME, DT, RAND, IN_GAME = 0x004e1df8, 0x0041f770, 0x00479d04, 0x00479cd4, 0x00423b30, 0x004e2650

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
app = p.read32(APP_POINTER)
fe = p.read32(app + 0x3d4)
event = p.read32(fe + 0xc)
stats = p.read32(app + 0x3e4)
print('activity %#x manager %#x cards %s app %#x' % (act, mgr, [hex(c) for c in cards], app), flush=True)


def snapshot(tag):
    profiles = [p.read32(app + 0x3f4 + 4 * k) for k in range(4)]
    return {
        'tag': tag, 'frame': p.read32(FRAME),
        'fame': p.read_block(p.read32(app + 0x3e0), 0x3ab4).hex(),
        'app': p.read_block(app + 0x3d0, 0x40).hex(),
        'fe': p.read_block(fe, 0x20).hex(),
        'event': p.read_block(event, 0x120).hex(),
        'profiles': profiles,
        'profile_slots': [p.read32(a + 0xcc) if a else None for a in profiles],
        'stats': p.read_block(stats, 0x200).hex(),
        'cards': cards, 'activity': act,
        # The track's podium flash levels (shortest +0x538, player win +0x53c, podium +0x544).
        'world': p.read_block(p.read32(act + 0x180) + 0x530, 0x20).hex(),
    }


out = open(out_path, 'w')
out.write(json.dumps(snapshot('before')) + '\n')
regions = [(FRAME, 4), (DT, 4), (RAND, 8), (mgr, 0x110)] + [(c, 0x1e0) for c in cards] + [(act, 0x4e0), (stats + 0x160, 0x40)]
poked, last, rows, skipped = None, p.read32(FRAME), 0, 0
while True:
    f = p.next_frame(last, addr=FRAME)
    last = f
    b = p.settle(regions, f)
    if b is None:
        skipped += 1
        continue
    row = {'frame': int.from_bytes(b[0:4], 'little'), 'dt': b[4:8].hex(), 'rand': b[8:16].hex()}
    at = 16
    for name, n in [('mgr', 0x110)] + [('card%d' % k, 0x1e0) for k in range(4)] + [('act', 0x4e0), ('stats', 0x40)]:
        row[name] = b[at:at + n].hex()
        at += n
    flash = int.from_bytes(bytes.fromhex(row['act'])[0x414:0x418], 'little')
    menu = int.from_bytes(bytes.fromhex(row['act'])[0x3b8:0x3bc], 'little')
    if flash:
        row['flash'] = p.read_block(flash, 0x70).hex()
    if menu:
        row['menu'] = p.read_block(menu, 0xd0).hex()
    out.write(json.dumps(row) + '\n')
    rows += 1
    flags = [int.from_bytes(bytes.fromhex(row['card%d' % k])[8:16], 'little') for k in range(4)]
    done = sum(1 for k in range(1, 4) if flags[k] & 1)
    if poked is None and done >= after:
        p.pause()
        v = p.read64(cards[0] + 8)
        p.write64(cards[0] + 8, v | 1)
        poked = p.read32(FRAME)
        p.resume()
        out.write(json.dumps({'poke': poked, 'flags_before': v, 'ai_done': done}) + '\n')
        print('poked at frame', poked, 'with', done, 'AI done', flush=True)
    if poked is not None and f - poked >= tail:
        break
out.write(json.dumps(snapshot('after')) + '\n')
out.close()
print('rows', rows, 'skipped', skipped, flush=True)
