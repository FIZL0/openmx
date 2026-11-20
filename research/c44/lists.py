"""C44: print a c44-plane-bike capture's AI bike's collision list per tick, as the frame left it.

usage: python3 -B research/c44/lists.py CSV PREFIX [FROM [TO]]

The record's count reads 0 at the frame's end (cleared), so each tick's entries are shown when they
differ from the tick before: kind, other (the plane's record is 0x01e73b00), mover, normal.
"""
import csv
import sys

pre = sys.argv[2]
lo = int(sys.argv[3]) if len(sys.argv) > 3 else 0
hi = int(sys.argv[4]) if len(sys.argv) > 4 else 1 << 40
last = None
for r in csv.DictReader(open(sys.argv[1])):
    f = int(r['frame_counter'])
    if not lo <= f <= hi:
        continue
    es = []
    for k in range(11):
        c = '%s_c%d.' % (pre, k)
        es.append('%s/%s/%s n(%.3f %.3f %.3f)' % (r[c + 'kind'], hex(int(r[c + 'other'])), hex(int(r[c + 'mover'])),
                                                  *(float(r[c + 'normal.' + a]) for a in 'xyz')))
    sig = tuple(es)
    print(f, 'count', r[pre + '_rec_count'], 'struck', r[pre + '_rec_struck'], 'shock', r[pre + '_ch_shock.x'],
          r[pre + '_ch_shock.z'], '' if sig != last else '(same list)')
    if sig != last:
        for k, e in enumerate(es):
            if not e.startswith('0/0x0/0x0'):
                print('   ', k, e)
    last = sig
