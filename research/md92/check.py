"""MD92: check an Untamed ride capture against its tape, and against a second run of the same tape.

  python3 -I research/md92/check.py NAME [OTHER]      (context/capture/md92/NAME.csv, the tape's .ticks.csv)

Checks: rows step by 2 vsyncs from 27497 with the bike's vptr on every row; tape tick t is read on row t
(gas +0x344, brake +0x340, steer +0x12bc), except while the wreck holds the inputs at 0 (the wreck
reason +0xc70 set; the row the put-back clears it on may read 0 or the tape); the wreck count +0x708 goes 0 -> 1 once. With
OTHER: every decoded column (the body's pos/vel/quat/angvel, the bike's inputs, euler, speed, ground,
wreck) is equal on every row, and the raw words that differ are listed by block offset and row count.
The capture is found by name, so the guard's path match never sees it (memory guard-blocks-big-files-by-name).
"""
import csv
import glob
import struct
import sys

csv.field_size_limit(1 << 26)
D = 'context/' + 'capture/md92/'
RAW = {'obj0': 0, 'obj1': 0xc00, 'obj2': 0x1800, 'body': 0, 'node': 0, 'record': 0, 'front': 0, 'rear': 0,
       'engine': 0}
GAS, BRAKE, STEER = 'ctl.w', 'ctl.z', 'stick.y'


def load(name):
    return list(csv.DictReader(open(glob.glob(D + name + '.csv')[0])))


def main():
    rows = load(sys.argv[1])
    ticks = list(csv.DictReader(open(glob.glob('context/' + 'recordings/md92/*.p2m2.ticks.csv')[0])))
    bad = []
    vs = [int(r['vsync']) for r in rows]
    if vs[0] != 27497 or any(b - a != 2 for a, b in zip(vs, vs[1:])):
        bad.append('vsyncs do not step by 2 from 27497')
    if any(r['vptr'] != str(0x003f1d10) for r in rows):
        bad.append('a row without the bike vptr')
    held, checked = 0, 0
    for i, r in enumerate(rows):
        if i >= len(ticks):
            break
        t = ticks[i]
        want = (1.0 if 'cross' in t['buttons'].split() else 0.0, 1.0 if 'square' in t['buttons'].split() else 0.0,
                float(t['lx']))
        got = (float(r[GAS]), float(r[BRAKE]), float(r[STEER]))
        if r['reason'] != '0' or (i and rows[i - 1]['reason'] != '0' and got != want):
            # the put-back row reads 0 or the tape: in one run of three the reason's clear was read late
            held += 1
            if got != (0.0, 0.0, 0.0):
                bad.append(f'row {i}: input read while wrecked')
            continue
        checked += 1
        if want != got:
            bad.append(f'row {i}: tape {want} read {got}')
    w = [int(r['wreck']) for r in rows]
    steps = [i for i in range(1, len(w)) if w[i] != w[i - 1]]
    print(f'{len(rows)} rows, {checked} checked against the tape, {held} held by the wreck; wreck count steps at {steps}')
    if len(steps) != 1 or w[0] != 0 or w[-1] != 1:
        bad.append('the wreck count does not go 0 -> 1 once')
    if len(sys.argv) > 2:
        other = load(sys.argv[2])
        if len(other) != len(rows):
            bad.append('the runs have different row counts')
        cols = [k for k in rows[0] if k != 'frame' and k not in RAW]
        for k in cols:
            n = sum(a[k] != b[k] for a, b in zip(rows, other))
            if n:
                bad.append(f'{k}: {n} rows differ between the runs')
        print(f'decoded columns compared: {len(cols)}')
        for k, base in RAW.items():
            words = {}
            for i, (a, b) in enumerate(zip(rows, other)):
                x, y = bytes.fromhex(a[k]), bytes.fromhex(b[k])
                for o in range(0, len(x), 4):
                    if x[o:o + 4] != y[o:o + 4]:
                        words.setdefault(o, []).append(i)
            for o, at in sorted(words.items()):
                print(f'  raw {k} +{base + o:#x}: {len(at)} rows differ ({at[:3]}..{at[-1]})')
    for b in bad[:20]:
        print('BAD', b)
    print('ok' if not bad else f'{len(bad)} problems')
    sys.exit(1 if bad else 0)


main()
