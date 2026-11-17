"""C40: read a capture from capture.sh: the trigger, the firing row, the gate's y per tick from there,
the clock, and the collision pieces' matrices (+0x1a0) before and after.

usage: summary.py CSV
"""
import csv, struct, sys

rows = list(csv.DictReader(open(sys.argv[1])))


def mat(raw, off):
    b = bytes.fromhex(raw)[off:off + 64]
    return [tuple(round(v, 4) for v in struct.unpack_from('<4f', b, 16 * r)) for r in range(4)]


r0 = rows[0]
print('rows', len(rows), 'frames', r0['frame_counter'], '..', rows[-1]['frame_counter'])
print('trigger', r0['trigger'], 'pieces', r0['pieces'], 'len', r0['sink_len'])
for k in range(2):
    print('col%d node(+0x230)' % k, r0['col%d_node' % k], 'matrix +0x1a0', mat(r0['col%d_raw' % k], 0x80))
fire = next((i for i, r in enumerate(rows) if r['lowered'] == '1'), None)
if fire is None:
    print('never lowered; last count', rows[-1]['count'])
    cards = [(r['frame_counter'], [(r['card%d_id' % k], r['card%d_kept' % k], r['card%d_now' % k]) for k in range(4)]) for r in rows[-3:]]
    print(cards)
    sys.exit()
print('lowered at row', fire)
for r in rows[max(0, fire - 3):fire + 12]:
    cards = [(r['card%d_id' % k], r['card%d_kept' % k], r['card%d_now' % k]) for k in range(4)]
    print(r['frame_counter'], 'dt', r['dt'], 'y', r['gate_y'], hex(int(r['gate_y_bits'])), 't', r['sink_t'],
          'on', r['sink_on'], 'armed', r['armed'], 'count', r['count'], cards)
for k in range(2):
    for i in (fire - 1, fire, fire + 1, fire + 9):
        if 0 <= i < len(rows):
            r = rows[i]
            print('col%d' % k, r['frame_counter'], '+0x1a0', mat(r['col%d_raw' % k], 0x80), '+0x1e0', mat(r['col%d_raw' % k], 0xc0)[3])
