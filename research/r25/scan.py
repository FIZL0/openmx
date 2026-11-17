"""R25: list every capture row whose collision records hold an entry naming a thrown blade.

usage: scan.py NAME  (context/capture/r25/NAME.csv, captured with watch/r25-kiowa-blades.toml)

Entries are 0x80 bytes: kind word +0x00 (0 = the record's own ray struck, 1 = a mirror), the other
side's record +0x74. Each record is named off the row's heli_record / bladeK_record words. Prints the
heli's entries on a blade (H) and a blade's on another blade (B), and a summary.
"""
import csv
import struct
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
rows = list(csv.DictReader(open(os.path.join(ROOT, 'context', 'capture', 'r25', sys.argv[1] + '.csv'))))
names = ['heli'] + [f'blade{k}' for k in range(4)]
seen = {'H0': [], 'H1': [], 'B0': [], 'B1': [], 'over': []}
for r in rows:
    frame = int(r['frame_counter'])
    rec = {}
    for n in names:
        v = r.get(f'{n}_record', '')
        if v:
            rec[int(v)] = n
    for n in names:
        cnt = r.get(f'{n}_contacts_n', '')
        raw = r.get(f'{n}_contacts', '')
        if not cnt or not raw:
            continue
        cnt = int(cnt)
        raw = bytes.fromhex(raw)
        if cnt > len(raw) // 0x80:
            seen['over'].append((frame, n, cnt))
        for k in range(min(cnt, len(raw) // 0x80)):
            kind, = struct.unpack_from('<I', raw, k * 0x80)
            other, = struct.unpack_from('<I', raw, k * 0x80 + 0x74)
            o = rec.get(other)
            if o is None or not o.startswith('blade'):
                continue
            tag = ('H' if n == 'heli' else 'B') + str(kind)
            if tag in seen:
                seen[tag].append((frame, n, k, o))
                if kind == 0:
                    print(frame, n, 'entry', k, 'kind', kind, 'on', o)
print('frames', rows[0]['frame_counter'], '..', rows[-1]['frame_counter'], 'max heli entries', max(int(r['heli_contacts_n'] or 0) for r in rows))
for t, v in seen.items():
    print(t, len(v), v[:6])
