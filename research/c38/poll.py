"""C38: watch the five quads' collision records of MD166's ATV race state, one line per game frame.

    KIT_CONFIG=<SLUS-21104 kit.toml> KIT_PCSX2=N ./kit pcsx2 with -- python3 -B research/c38/poll.py FRAMES [SAVE_FROM SAVE_TO]

Run it with the race state loaded. Each line: the frame (vsync 0x3fa6d8), then per record its contact count
and the records its entries name (+0x84). With SAVE_FROM/SAVE_TO, every frame in that range is saved into
slots 10.. in turn (run the copy slowly, NominalScalar ~0.05, so a save fits in a frame); the saved
states carry their own frame counter, so which frame each holds is read back from the file, not assumed.
"""
import struct
import sys
import time

sys.path.insert(0, '.')
from ps2kit.pine import Pine  # noqa: E402

VSYNC = 0x3FA6D8
# MD166's atv-race.p2s: quads 0x1551620 (player), 0x15ca4b0, 0x16219d0, 0x1a0fc70, 0x1a65ba0; record =
# *(*(quad + 0x1434) + 0x90) (the wreck part's record): count +0xf0, list +0xf8, 0x90 an entry, +0x84 other.
RECORDS = [0x159BA10, 0x160D130, 0x1663830, 0x1A511B0, 0x1AA5020]


def snapshot(p):
    out = []
    for r in RECORDS:
        n, _, lst = struct.unpack('<III', p.read_block(r + 0xF0, 12))
        others = []
        for i in range(min(n, 26)):
            e = lst + i * 0x90
            kind = p.read32(e)
            others.append((kind, p.read32(e + 0x84)))
        out.append((n, others))
    return out


def main():
    frames = int(sys.argv[1])
    lo, hi = (int(sys.argv[2]), int(sys.argv[3])) if len(sys.argv) > 3 else (None, None)
    p = Pine()
    f = p.frame(VSYNC)
    slot = 10
    deadline = time.time() + 3600
    for _ in range(frames):
        f = p.next_frame(f, addr=VSYNC)
        s = snapshot(p)
        if p.frame(VSYNC) != f:
            print(f, 'torn', flush=True)
            continue
        cols = []
        for r, (n, others) in zip(RECORDS, s):
            named = ','.join(f'{k}:{o:x}' for k, o in others if o)
            cols.append(f'{r:x}={n}' + (f'[{named}]' if named else ''))
        print(f, ' '.join(cols), flush=True)
        if lo is not None and lo <= f <= hi:
            p.save_state(slot)
            print(f, 'saved slot', slot, flush=True)
            slot += 1
        if time.time() > deadline:
            break


main()
