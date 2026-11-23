"""S504: steer the 2p state's riders into each other with no pad: grow a two-pad tape a few ticks at a
time, playing what there is (research/s158/scout.sh's steps), reading where it leaves both riders over
PINE, and choosing the next ticks' pads from that. The tape is all that is kept: nothing is written to
the game. Research, not product.

    ./kit pcsx2 with -- python3 -B research/s504/pilot.py PID SCENARIO [--step N] [--ticks N] [--keep K]

Writes context/recordings/split/s504/SCENARIO.p1.pad / .p2.pad (tape2.py's pad scripts) and prints a line
a step. --keep K starts from the first K ticks of the pads already there. `SCENARIO play` in place of
the options only plays the pads and prints every tick read.
"""
import argparse
import math
import os
import shutil
import struct
import subprocess
import sys
import time

sys.path.insert(0, os.getcwd())
from ps2kit.pine import Pine  # noqa: E402

import scenarios  # noqa: E402

C6 = 'research/journal/2026-10-20-c6-c6-recording2-capture/playback'
DIR = 'context/recordings/split'
GAME_MANAGER, FRAME_COUNTER = 0x004e1e98, 0x00479d04
FIRST = 127882                      # the frame the tape's first tick steps
CHAINS = ([0x34, 0x34, 0x8, 0x8, 0x8, 0x0], [0x34, 0x34, 0x8, 0x8, 0x0])


class Rider:
    """One rider as RAM holds it: centre, velocity, forward, wreck reason and collision entries."""

    def __init__(self, pine, hops):
        a = pine.read32(GAME_MANAGER)
        for h in hops:
            a = pine.read32(a + h)
        self.rec = pine.read32(a + 0x7d0)
        ch = pine.read32(a + 0x7a4)
        self.wreck = pine.read32(a + 0x748)
        self.pos = struct.unpack('<3f', pine.read_block(ch + 0x60, 12))
        self.vel = struct.unpack('<3f', pine.read_block(ch + 0x260, 12))
        node = pine.read32(self.rec + 0xf8)
        self.fwd = struct.unpack('<3f', pine.read_block(node + 0xa0, 12))
        n, at = pine.read32(self.rec + 0x100), pine.read32(self.rec + 0x108)
        self.hits = [(pine.read32(at + 0x80 * k), pine.read32(at + 0x80 * k + 0x74)) for k in range(min(n, 11))]

    @property
    def speed(self):
        return math.hypot(*self.vel)

    @property
    def heading(self):
        v = self.vel if math.hypot(self.vel[0], self.vel[2]) > 12 else self.fwd
        return math.atan2(v[0], v[2])

    def __str__(self):
        hit = ' HIT%s' % [(k, hex(o)) for k, o in self.hits if o] if any(o for _, o in self.hits) else ''
        return '%8.1f %6.1f %8.1f v%5.1f h%7.1f f%7.1f w%d%s' % (
            *self.pos, self.speed, math.degrees(self.heading),
            math.degrees(math.atan2(self.fwd[0], self.fwd[2])), self.wreck, hit)


def script(pads):
    out, last = ['release'], None
    for p in pads:
        if p == last:
            out[-1] = 'sleep %dt' % (int(out[-1].split()[1][:-1]) + 1)
            continue
        gas, brake, x, y = p
        out.append('release')
        if gas:
            out.append('down cross %.3f' % gas)
        if brake:
            out.append('down square')
        out += ['stick l %.3f %.3f' % (x, y), 'sleep 1t']
        last = p
    return '\n'.join(out) + '\n'


def unscript(path):
    pads, gas, brake, x, y = [], 0, 0, 0, 0
    for line in open(path):
        w = line.split('#')[0].split()
        if not w:
            continue
        if w[0] == 'release':
            gas = brake = x = y = 0
        elif w[:2] == ['down', 'cross']:
            gas = float(w[2]) if len(w) > 2 else 1.0
        elif w[:2] == ['down', 'square']:
            brake = 1
        elif w[0] == 'stick':
            x, y = float(w[2]), float(w[3])
        elif w[0] == 'sleep':
            pads += [(gas, brake, x, y)] * int(w[1][:-1])
    return pads


def gui(pid, name, *args, wait=True):
    cmd = ['python3', '-B', f'{C6}/{name}.py', str(pid), *args]
    run = subprocess.run if wait else subprocess.Popen
    return run(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def play(pine, pid, name, until, echo=False):
    """Play context/recordings/split/s504/NAME.p2m2 and read both riders each tick up to `until`."""
    work = '/run/user/1000/s504'
    os.makedirs(work, exist_ok=True)
    rec = f'{work}/play.p2m2'
    for p in (rec, rec + '_SaveState.p2s'):
        if os.path.exists(p):
            os.remove(p)
    shutil.copyfile(f'{DIR}/s504/{name}.p2m2', rec)
    shutil.copyfile(f'{DIR}/2p.p2m2_SaveState.p2s', rec + '_SaveState.p2s')
    gui(pid, 'press', 'Tools/Input Recording/Stop')
    gui(pid, 'resume_if_paused')
    gui(pid, 'press', 'System/Reset')
    time.sleep(1.5)
    gui(pid, 'press', 'Tools/Input Recording/Play', wait=False)
    gui(pid, 'open_only', rec)
    time.sleep(2)
    gui(pid, 'resume_if_paused')
    rows, last, end = [], None, time.time() + 90
    while time.time() < end:
        try:
            f = pine.read32(FRAME_COUNTER)
            if f == last or not FIRST <= f < FIRST + 100000:
                time.sleep(0.001)
                continue
            row = (f - FIRST, Rider(pine, CHAINS[0]), Rider(pine, CHAINS[1]))
        except SystemExit:
            raise
        except Exception:  # noqa: BLE001 - a load, or a chain torn mid-frame
            time.sleep(0.02)
            continue
        last = f
        rows.append(row)
        if echo:
            print(f, row[1], '|', row[2], '| %.1f' % math.dist(row[1].pos, row[2].pos), flush=True)
        if f - FIRST >= until:
            break
    gui(pid, 'press', 'Tools/Input Recording/Stop')
    return rows


def write(name, pads):
    os.makedirs(f'{DIR}/s504', exist_ok=True)
    for p in (0, 1):
        open(f'{DIR}/s504/{name}.p{p + 1}.pad', 'w').write(script(pads[p]))
    subprocess.run(['python3', '-I', 'research/s158/tape2.py', f'{DIR}/2p.p2m2', f'{DIR}/s504/{name}.p1.pad',
                    f'{DIR}/s504/{name}.p2.pad', f'{DIR}/s504/{name}.p2m2'], check=True, stdout=subprocess.DEVNULL)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('pid', type=int)
    ap.add_argument('scenario')
    ap.add_argument('mode', nargs='?')
    ap.add_argument('--step', type=int, default=20)
    ap.add_argument('--ticks', type=int, default=600)
    ap.add_argument('--keep', type=int, default=0)
    a = ap.parse_args()
    base = f'{DIR}/s504/{a.scenario}'
    pine = Pine()
    if a.mode == 'play':
        rows = play(pine, a.pid, a.scenario, len(unscript(base + '.p1.pad')) - 2, echo=True)
        return
    policy = getattr(scenarios, a.scenario)
    pads = [[], []]
    if a.keep:
        pads = [unscript(f'{base}.p{p}.pad')[:a.keep] for p in (1, 2)]
    else:
        pads = [[(0, 0, 0, 0)] * 10, [(0, 0, 0, 0)] * 10]
    while len(pads[0]) < a.ticks:
        t = len(pads[0])
        write(a.scenario, [p + [p[-1]] * 120 for p in pads])
        rows = play(pine, a.pid, a.scenario, t)
        hit = next((r for r in rows if any(o in (r[1].rec, r[2].rec) for q in r[1:] for _, o in q.hits)), None)
        if hit:
            print('CONTACT', FIRST + hit[0], hit[1], '|', hit[2], flush=True)
            for r in rows:
                if abs(r[0] - hit[0]) <= 4:
                    print(r[0], r[1], '|', r[2])
            write(a.scenario, [p + [p[-1]] * 120 for p in pads])
            return
        k, r1, r2 = rows[-1]
        print(k, r1, '|', r2, '| %.1f' % math.dist(r1.pos, r2.pos), flush=True)
        if r1.wreck or r2.wreck:
            print('wrecked')
            return
        for p, pad in zip(pads, policy(k, r1, r2)):
            p += [pad] * a.step


if __name__ == '__main__':
    main()
