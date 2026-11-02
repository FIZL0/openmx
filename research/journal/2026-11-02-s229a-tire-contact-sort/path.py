"""S229a: a capture's path, every STEP rows: frame_counter, the body's position, yaw (quat y, w), speed, each
tire's solid (+0xb4), sorted (+0xb8) and struck object (+0x1ac, o when set), and the sort probe's hit count
and flag byte (+0x116).

usage: path.py CSV [STEP]
"""
import csv
import struct
import sys

rows = list(csv.DictReader(open(sys.argv[1])))
step = int(sys.argv[2]) if len(sys.argv) > 2 else 10
for i, x in enumerate(rows):
    if i % step and i != len(rows) - 1 and not (len(sys.argv) > 3 and any(
            struct.unpack_from('<I', bytes.fromhex(x[f'tire_{n}_raw']), 0xb8)[0] for n in ('fl', 'fr', 'rl', 'rr'))):
        continue
    tires = []
    for n in ('fl', 'fr', 'rl', 'rr'):
        h = bytes.fromhex(x[f'tire_{n}_raw'])
        w = lambda o: struct.unpack_from('<I', h, o)[0]
        tires.append(f"{w(0xb4)}{w(0xb8)}{'o' if w(0x1ac) else '-'}")
    count = struct.unpack_from('<i', bytes.fromhex(x['sort_list_raw']), 0xc)[0]
    flags = bytes.fromhex(x['sort_probe_raw'])[0x116]
    speed = (float(x['body_vel.x']) ** 2 + float(x['body_vel.z']) ** 2) ** 0.5
    print(x['frame_counter'], [round(float(x['body_pos.' + k]), 1) for k in 'xyz'],
          [round(float(x['body_quat.' + k]), 3) for k in 'yw'], round(speed, 1), ' '.join(tires),
          'hits', count, hex(flags))
