"""N24: the two-plane capture's frames, gaps and touch (capture/n24/meet under context/, the csv).

    python3 -B research/n24/events.py [FROM TO]    # rows FROM..TO (default: the touch's start)

Prints the frame span and gaps, who flies each plane, and per row both planes' velocities, their
centres' distance and the crash words. Research, not product.
"""
import csv, math, os, sys

r = list(csv.DictReader(open(os.path.join('context', 'capture', 'n24', 'meet' + '.csv'))))
fr = [int(x['frame']) for x in r]
print('rows', fr[0], '..', fr[-1], len(fr), 'gaps', [b for a, b in zip(fr, fr[1:]) if b != a + 1][:5])
print('ctl_src after 138000: plane0', sorted({x['plane_ctl_src'] for x in r if int(x['frame']) > 138000}),
      'plane1', sorted({x['plane1_plane_ctl_src'] for x in r if int(x['frame']) > 138000}))
lo, hi = (int(sys.argv[1]), int(sys.argv[2])) if len(sys.argv) > 2 else (138195, 138240)
v = lambda x, p: [float(x[p + a]) for a in 'xyz']
for x in r:
    f = int(x['frame'])
    if lo <= f <= hi:
        d = math.dist(v(x, 'body_pos.'), v(x, 'plane1_body_pos.'))
        print(f, 'v0 %8.3f %7.3f %7.3f' % tuple(v(x, 'body_vel.')), 'v1 %8.3f %7.3f %7.3f' % tuple(v(x, 'plane1_body_vel.')),
              'd %6.2f' % d, 'crash', x['crash'], x['plane1_crash'], 'dt', x['dt'])
