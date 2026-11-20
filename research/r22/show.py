"""R22: print a watch/r22-split-car.toml capture: every tenth row, and every row whose bike record holds
contacts (each as kind / other record / normal).

    python3 -I research/r22/show.py context/capture/r22/<name>.csv [FROM TO]
"""
import csv, sys

rows = list(csv.DictReader(open(sys.argv[1])))
lo, hi = (int(sys.argv[2]), int(sys.argv[3])) if len(sys.argv) > 3 else (0, 1 << 31)
f = lambda x, k: '%9.3f' % float(x[k])
for i, x in enumerate(rows):
    fc = int(x['frame_counter'])
    n = min(int(x['rec_count']), 11)
    if not lo <= fc <= hi or not (n or i % 10 == 0 or lo):
        continue
    c = ['%s/%s/(%.2f %.2f %.2f)' % (x['c%d.kind' % k], x['c%d.other' % k], float(x['c%d.normal.x' % k]),
                                     float(x['c%d.normal.y' % k]), float(x['c%d.normal.z' % k])) for k in range(n)]
    print(fc, 'bike', f(x, 'ch_cofg.x'), f(x, 'ch_cofg.z'), 'v', f(x, 'ch_vel.x'), f(x, 'ch_vel.z'), 'n', n,
          x['rec_struck'][:16], 'car', f(x, 'car_pos.x'), f(x, 'car_pos.z'), 'v', f(x, 'car_vel.x'), f(x, 'car_vel.z'),
          ' '.join(c))
    if 'car_rec_count' in x:
        m = min(int(x['car_rec_count']), 11)
        print('      car record', m, x['car_rec_struck'], ' '.join(
            '%s/%s/(%.2f %.2f %.2f)' % (x['k%d.kind' % k], x['k%d.other' % k], float(x['k%d.normal.x' % k]),
                                        float(x['k%d.normal.y' % k]), float(x['k%d.normal.z' % k])) for k in range(m)))
