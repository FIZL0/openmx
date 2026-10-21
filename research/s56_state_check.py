"""S56: is a save state one whole tick of an s56 capture? Compares every bone's body in the state's RAM
against the capture row with the state's frame counter, bit for bit.

usage: python3 -B research/s56_state_check.py NAME STATE

STATE is a slot number or a .p2s path. A state PCSX2 saved while the game was between ticks agrees
with its row in every lane; one saved mid-tick agrees with neither its row nor the next.
"""

import csv
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from ps2kit.config import cfg          # noqa: E402
from ps2kit.emu import state_ee        # noqa: E402

BIKE = (0x34, 0x34) + (0x8,) * 8 + (0x0,)
FIELDS = [('last_pos', 0x50), ('pos', 0x60), ('vel', 0x70), ('last_quat', 0x150), ('quat', 0x170), ('rates', 0x1b0)]


def main():
    name, state = sys.argv[1], sys.argv[2]
    ram = open(state_ee(state), 'rb').read()
    u32 = lambda a: struct.unpack_from('<I', ram, a & 0x1ffffff)[0]
    f32 = lambda a: struct.unpack_from('<f', ram, a & 0x1ffffff)[0]
    syms = cfg().symbols
    frame = u32(syms['frame_counter'])
    p = syms['game_manager']
    for off in BIKE:
        p = u32(p) + off
    body = u32(p) - 0x400
    parts = u32(body + 0x2f4)
    rows = {int(r['frame_counter']): r for r in csv.DictReader(open(f'context/capture/s56/{name}.csv'))}
    print(f'state frame {frame}, body actor {body:#x}, riding {u32(body + 0x2a4)}')
    for at in (frame - 1, frame, frame + 1):
        r = rows.get(at)
        if r is None:
            print(f'  no row for frame {at}')
            continue
        bad = 0
        for i in range(18):
            b = u32(parts + 0xb0 * i + 0xa4)
            for fname, off in FIELDS:
                for k, lane in enumerate('xyzw'):
                    got = f32(b + off + 4 * k)
                    want = float(r[f'bone{i}.{fname}.{lane}'])
                    if struct.pack('<f', got) != struct.pack('<f', want):
                        bad += 1
        print(f'  row frame {at}: {bad} lanes differ of {18 * len(FIELDS) * 4}')


main()
