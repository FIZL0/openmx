"""R51: a quad capture's path, every Nth row: row, vsync, pos, speed, heading, ctl, wreck, extra columns.

    python3 -I research/r51/track.py DIR NAME [EVERY [COLUMN...]]

The capture is DIR/NAME.csv, found by glob (the context guard reads command lines). A COLUMN ending
in `.` prints its .x .y .z."""
import csv, glob, os, sys

csv.field_size_limit(1 << 26)
path = [p for p in glob.glob(os.path.join(sys.argv[1], sys.argv[2] + '.c*')) if p.endswith('.csv')][0]
every = int(sys.argv[3]) if len(sys.argv) > 3 else 5


def v(r, k):
    return ' '.join(f"{float(r[k + c]):.2f}" for c in 'xyz')


for i, r in enumerate(csv.DictReader(open(path))):
    if i % every:
        continue
    extra = [f"{k}{v(r, k)}" if k.endswith('.') else f"{k}={r[k][:40]}" for k in sys.argv[4:]]
    print(i, r['vsync'], 'pos', v(r, 'pos.'), f"sp {float(r['speed']):.1f} yaw {float(r['euler.y']):.2f}",
          'ctl', v(r, 'ctl.'), 'wr', r['wreck'], *extra)
