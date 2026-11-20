"""C43: which csv row each RAM image holds. The helicopter's body (0x00606f30, 0x200 bytes) and the prop's
(0x00e301a0) in the image against every row's body_raw and prop_body_raw; prints the rows that match whole.
Also compares two csvs of the same tape, lane by lane (the raw blocks included), when --against is given.

usage: python3 -I research/c43/images.py CSV IMAGE... [--against CSV2]
"""
import csv
import sys

HELI_BODY, PROP_BODY = 0x00606f30, 0x00e301a0
args = sys.argv[1:]
other = None
if '--against' in args:
    i = args.index('--against')
    other = args[i + 1]
    del args[i:i + 2]
rows = list(csv.DictReader(open(args[0])))


def raw(s):
    return bytes.fromhex(s.replace(' ', ''))


for path in args[1:]:
    with open(path, 'rb') as f:
        f.seek(HELI_BODY)
        heli = f.read(0x200)
        f.seek(PROP_BODY)
        prop = f.read(0x200)
    hits = [(r['frame_counter'], raw(r['prop_body_raw']) == prop) for r in rows if raw(r['body_raw']) == heli]
    print(path.rsplit('/', 1)[-1], 'heli body = rows', hits or 'none',
          '| prop enabled', int.from_bytes(prop[:4], 'little'))

if other:
    b = {r['frame_counter']: r for r in csv.DictReader(open(other))}
    diff = {}
    same = 0
    for r in rows:
        o = b.get(r['frame_counter'])
        if o is None:
            continue
        lanes = [k for k in r if k not in ('frame',) and not k.startswith(('pad_', 'stamp_')) and r[k] != o[k]]
        if lanes:
            diff[r['frame_counter']] = lanes
        else:
            same += 1
    print(same, 'rows the same in both (pads and stamps aside);', len(diff), 'differ')
    lanes = {}
    for v in diff.values():
        for k in v:
            lanes[k] = lanes.get(k, 0) + 1
    print('rows differing per lane:', lanes)
    for k, v in list(diff.items())[:20]:
        print(' ', k, v[:8])
