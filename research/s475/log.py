"""S475: per-frame log of the free09 beach balls' bodies and the bike while a tape plays.
usage: log.py OUT.csv FRAMES
Columns: the frame counter, the bike's chassis position (a put-back is a jump of tens of feet in one
row), then each of the five BeachBall bodies (first_recording's heap, as research/s195/knock.py):
enabled word (0 asleep) and position."""
import struct
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(ROOT))
from ps2kit import pine as pine_mod      # noqa: E402

BODIES = [0x11b6030, 0x11b6a90, 0x11bf460, 0x11c1f50, 0x11c4a40]
VS = 0x479d04
GM = 0x004e1e98
RIDER = (0x34, 0x34, 0x8, 0x8, 0x0)

out, n = sys.argv[1], int(sys.argv[2])
# POKE_AT: the first frame at or after it moves every asleep ball's body 10 ft along x (a prop off its spot
# that nothing will move again: an asleep body is never stepped, and a never-woken one counts no ResetTime).
poke_at = int(sys.argv[3]) if len(sys.argv) > 3 else None
p = pine_mod.Pine(check=False, timeout=60)


def chassis():
    q = p.read32(GM)
    for off in RIDER:
        if not 0 < q < 0x0200_0000:
            return 0
        q = p.read32(q + off)
    return p.read32(q + 0x7a4) if 0 < q < 0x0200_0000 else 0


with open(out, 'w') as f:
    f.write('wall,vsync,cx,cy,cz,' + ','.join(f'b{i}on,b{i}x,b{i}y,b{i}z' for i in range(5)) + '\n')
    last, t0 = -1, time.time()
    for _ in range(n):
        v = p.next_frame(last, addr=VS)
        last = v
        if poke_at is not None and v >= poke_at:
            for b in BODIES:
                if p.read32(b) == 0:
                    p.write_f32(b + 0x60, p.read_f32(b + 0x60) + 10.0)
            poke_at = None
        ch = chassis()
        c = struct.unpack('<3f', p.read_block(ch + 0x60, 12)) if ch else (0, 0, 0)
        row = [f'{time.time() - t0:.2f}', str(v)] + [f'{x:.3f}' for x in c]
        for b in BODIES:
            row.append(str(p.read32(b)))
            row += [f'{x:.3f}' for x in struct.unpack('<3f', p.read_block(b + 0x60, 12))]
        f.write(','.join(row) + '\n')
        f.flush()
