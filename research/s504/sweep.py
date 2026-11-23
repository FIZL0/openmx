"""S504: play variants of a pilot's tape: P's stick x over ticks A..B set to each value, printing the
contact each gives (frame, entries, the wrecks 60 ticks on). Run as pilot.py is. Research, not product.

    ./kit pcsx2 with -- python3 -B research/s504/sweep.py PID NAME P A B X...

Each variant is written as NAME_PA_B_X (pads and tape).
"""
import math
import sys

sys.argv, argv = [sys.argv[0]], sys.argv[1:]
sys.path.insert(0, 'research/s504')
import pilot  # noqa: E402

pid, name, p, a, b = int(argv[0]), argv[1], int(argv[2]), int(argv[3]), int(argv[4])
pine = pilot.Pine()
base = f'{pilot.DIR}/s504/{name}'
for x in argv[5:]:
    pads = [pilot.unscript(f'{base}.p{q}.pad') for q in (1, 2)]
    for t in range(a, b):
        g, br, _, y = pads[p - 1][t]
        pads[p - 1][t] = (g, br, float(x), y)
    out = f'{name}_{p}{a}_{b}_{x}'
    pilot.write(out, pads)
    rows = pilot.play(pine, pid, out, len(pads[0]) - 4)
    near = min(rows, key=lambda r: math.dist(r[1].pos, r[2].pos))
    print('==', out, 'nearest', near[0], '%.2f' % math.dist(near[1].pos, near[2].pos), 'wrecks at end', rows[-1][1].wreck, rows[-1][2].wreck)
    first = next((r for r in rows if any(o in (r[1].rec, r[2].rec) for q in r[1:] for _, o in q.hits)), None)
    for r in rows:
        if first and abs(r[0] - first[0] - 1) <= 3:
            print('  ', pilot.FIRST + r[0], r[1], '|', r[2])
    w = next((r for r in rows if r[1].wreck or r[2].wreck), None)
    if w:
        print('  first wreck', pilot.FIRST + w[0], w[1], '|', w[2], flush=True)
