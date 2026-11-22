#!/usr/bin/env python3
"""S90: the original's free09 ride (S66's 'ride' script, in game ticks), paused and shot at set ticks.

Run under ./kit pcsx2 with -- from the worktree. Writes context/shots/s90/pcsx2-<tick>.png and a log.
"""
import glob
import os
import shutil
import subprocess
import sys
import time

ROOT = os.getcwd()
sys.path.insert(0, ROOT)
from ps2kit.pine import Pine  # noqa: E402

FRAME_COUNTER = 0x00479D04
GAME_MANAGER = 0x004E1E98
# The kit copy's config folder (copy N lives in openmx-slots/pcsx2/sN/PCSX2).
COPY = os.environ.get('PCSX2_CONFIG', os.path.expanduser(f"~/repos/openmx-slots/pcsx2/s{os.environ.get('KIT_PCSX2', '2')}/PCSX2"))
SLOT = os.path.join(COPY, 'savestates', 'SLUS-20767 (9866FE54).08.p2s')
OUT = os.path.join(ROOT, 'context', 'shots', 's90')
SNAPS = os.path.join(COPY, 'snaps')
PRESS = os.path.join(ROOT, 'research/journal/2026-10-20-c6-c6-recording2-capture/playback/press.py')
PID = int(os.environ['PCSX2_PID'])
PAUSE = os.environ.get('SHOT_PAUSE', '1') == '1'

# S66's 'ride' in 30 Hz ticks: gas at 1.5 s, the put-back chord at 6.5 s, again 4.5 s later.
PAD = {0: ['release'], 45: ['down cross'], 195: ['down l1'], 199: ['down r1'], 205: ['up l1', 'up r1'],
       330: ['down l1'], 334: ['down r1'], 340: ['up l1', 'up r1'], 430: ['release']}
SHOTS = [60, 90, 96, 102, 120, 150, 200, 206, 212, 218, 225, 240, 260, 300, 335, 350, 410]
END = 440
if os.environ.get('SHOTS'):
    SHOTS = [int(s) for s in os.environ['SHOTS'].split(',')]
    END = SHOTS[-1] + 10


def chain(p, addr, *offs):
    for off in offs:
        addr = p.read32(addr)
        if not addr or addr < 0x100000 or addr >= 0x2000000:
            return 0
        addr += off
    return addr


def human_bike(p):
    node = p.read32(chain(p, GAME_MANAGER, 0x34, 0x30 + 4))
    for _ in range(64):
        if not node:
            break
        item = p.read32(node)
        if item and p.read32(item) == 2:
            player = p.read32(item + 0x20)
            if player and p.read32(player + 0x34) == 1:
                return item
        nxt = p.read32(node + 8)
        if nxt == node:
            break
        node = nxt
    return 0


def pad(cmds):
    subprocess.run([os.path.join(ROOT, 'kit'), 'pad', 'send', *cmds], cwd=ROOT, check=True)


def main():
    os.makedirs(OUT, exist_ok=True)
    src = glob.glob(os.path.join(ROOT, 'context', 'recordings', 'recording2.p2m2_SaveState.p2s'))[0]
    shutil.copyfile(src, SLOT)
    log = open(os.path.join(OUT, os.environ.get('LOG', 'pcsx2-log.txt')), 'w')
    p = Pine()
    pad(['release'])
    p.load_state(8)
    t0 = time.time()
    bike = 0
    while not bike and time.time() - t0 < 30:
        time.sleep(0.5)
        try:
            bike = human_bike(p)
        except Exception:
            bike = 0
    log.write(f'# bike found after {time.time() - t0:.1f} s, frame {p.read32(FRAME_COUNTER)}\n')
    if not bike:
        print('no human bike')
        return 1
    node = p.read32(bike + 0x18)
    f0 = p.read32(FRAME_COUNTER)
    prev = f0
    done_pad = set()
    shots = list(SHOTS)
    log.write(f'# bike {bike:08x} node {node:08x} f0 {f0}\n')
    while True:
        f = p.read32(FRAME_COUNTER)
        if f == prev:
            time.sleep(0.002)
            continue
        prev = f
        t = f - f0
        for k in sorted(PAD):
            if k <= t and k not in done_pad:
                done_pad.add(k)
                pad(PAD[k])
                log.write(f'pad {t} {PAD[k]}\n')
        x, y, z = (p.read_f32(node + 0xb0 + 4 * i) for i in range(3))
        clock = p.read_f32(bike + 0x794)
        ghost = p.read_f32(bike + 0x7dc)
        log.write(f'{t} pos {x:.3f} {y:.3f} {z:.3f} clock {clock:.3f} ghost {ghost:.3f}\n')
        if shots and t >= shots[0]:
            want = shots.pop(0)
            if PAUSE:
                p.pause()
            got = p.read32(FRAME_COUNTER) - f0
            before = set(os.listdir(SNAPS)) if os.path.isdir(SNAPS) else set()
            r = subprocess.run(['python3', '-B', PRESS, str(PID), 'System/Screenshot'],
                               cwd=ROOT, capture_output=True, text=True, timeout=30)
            new = []
            for _ in range(50):
                new = sorted(set(os.listdir(SNAPS)) - before) if os.path.isdir(SNAPS) else []
                if new:
                    break
                time.sleep(0.1)
            if new:
                time.sleep(0.3)
                shutil.move(os.path.join(SNAPS, new[-1]), os.path.join(OUT, f'pcsx2-{want}.png'))
            after = p.read32(FRAME_COUNTER) - f0
            log.write(f'shot {want} at tick {got}..{after}: {r.returncode} {new} {r.stderr.strip()[-200:]}\n')
            if PAUSE:
                p.resume()
            time.sleep(0.1)
        if t >= END:
            break
    pad(['release'])
    log.close()
    return 0


if __name__ == '__main__':
    sys.exit(main())
