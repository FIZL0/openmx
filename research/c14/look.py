"""C14: summarise a c14-kiowa capture: gaps, repeated rows, the pad and the flight every N rows.
usage: look.py CSV [EVERY]"""
import csv, sys
rows = list(csv.DictReader(open(sys.argv[1])))
every = int(sys.argv[2]) if len(sys.argv) > 2 else 30
fc = [int(r['frame_counter']) for r in rows]
print(len(rows), 'rows', fc[0], '..', fc[-1], 'gaps', sum(1 for a, b in zip(fc, fc[1:]) if b != a + 1))
key = lambda r: (r['body_pos.x'], r['body_pos.y'], r['body_pos.z'], r['body_quat.w'])
moving = lambda r: abs(float(r['body_vel.x'])) + abs(float(r['body_vel.y'])) + abs(float(r['body_vel.z'])) > 0.01
print('repeated moving rows', [fc[i] for i in range(1, len(rows)) if key(rows[i]) == key(rows[i - 1]) and moving(rows[i])])
print('src/class', sorted({(r['heli_ctl_src'], r['heli_class']) for r in rows}), 'dt', sorted({r['dt'] for r in rows}))
w = lambda r, off: int(r['heli_a'][2 * (off - 0x330):2 * (off - 0x330) + 8][::-1] and
                     bytes.fromhex(r['heli_a'][2 * (off - 0x330):2 * (off - 0x330) + 8])[::-1].hex(), 16)
for i, r in enumerate(rows):
    if i % every and not (i and (r['climb'], r['descend'], r['stick.x'], r['stick.y'], r['stick.z']) !=
                          (rows[i-1]['climb'], rows[i-1]['descend'], rows[i-1]['stick.x'], rows[i-1]['stick.y'], rows[i-1]['stick.z'])):
        continue
    f = lambda k: '%8.3f' % float(r[k])
    print(fc[i], 'in', r['climb'], r['descend'], r['stick.x'], r['stick.y'], r['stick.z'], '| cmd', f('commands.x'), f('commands.y'),
          f('commands.z'), f('commands.w'), '| pos', f('body_pos.x'), f('body_pos.y'), f('body_pos.z'), '| vel', f('body_vel.x'), f('body_vel.y'),
          f('body_vel.z'), '| wreck', hex(w(r, 0x3d4)), 'flown', w(r, 0x364))
