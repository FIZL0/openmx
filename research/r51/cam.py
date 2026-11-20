"""R51: read a camera.toml capture: every Nth row the quad's place, speed and pad words, the eye's goal and
position, the last clear goal, the contact ring's index, the boost bytes and the free look's yaw/pitch.

    python3 -I research/r51/cam.py NAME [EVERY [FIRST [LAST]]]

The capture is context/capture/r51/NAME.csv (found by glob, as the context guard reads command lines).
Rows where the goal is not the last clear goal, the ring moved or a boost byte is set are marked `*`."""
import csv, glob, os, struct, sys

csv.field_size_limit(1 << 26)
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
path = [p for p in glob.glob(os.path.join(ROOT, 'context', 'capture', 'r51', sys.argv[1] + '.c*'))
        if p.endswith('.csv')][0]
every = int(sys.argv[2]) if len(sys.argv) > 2 else 5
first = int(sys.argv[3]) if len(sys.argv) > 3 else 0
last = int(sys.argv[4]) if len(sys.argv) > 4 else 1 << 30


def v(r, k):
    return tuple(float(r[f'{k}.{c}']) for c in 'xyz')


def s(t):
    return ' '.join(f'{x:7.2f}' for x in t)


def words(r, k):
    return bytes.fromhex(r[k].replace(' ', ''))


ring0 = None
for i, r in enumerate(csv.DictReader(open(path))):
    cam = words(r, 'camera')
    yaw, pitch = struct.unpack_from('<2f', cam, 0x9c)
    ring = int(r['ring'])
    mark = v(r, 'eye_goal') != v(r, 'clear') or (ring0 is not None and ring != ring0) or int(r['boost']) & 0xffff
    ring0 = ring
    if first <= i <= last and (i % every == 0 or mark):
        print(f"{i:3d} {r['vsync']} {'*' if mark else ' '} pos {s(v(r, 'pos'))} sp {float(r['speed']):6.2f}",
              f"ctl {s(v(r, 'ctl'))} | goal {s(v(r, 'eye_goal'))} eye {s(v(r, 'eye'))} clear {s(v(r, 'clear'))}",
              f"ring {ring} boost {int(r['boost']):#x} yaw {yaw:.3f} pitch {pitch:.3f} wr {r['wreck']}")
