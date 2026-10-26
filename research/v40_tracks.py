"""V40: what the original's tire tracks read, live. Loads a state, rides a pad script and logs, ten times a
second, each wheel's surface zone (rider +0x910/+0x911), the fields the TireTrack entry gates on and
the chassis speed, then takes a screenshot.

usage: python3 -B research/v40_tracks.py P2S PAD SECONDS SHOT

Run it under `./kit pcsx2 with --` on copy 1. P2S is copied over slot 8 and loaded; PAD is a `./kit pad
send` script file played alongside; SHOT is where `./kit shot` writes the frame at the end.
"""
import shutil, struct, subprocess, sys, time
sys.path.insert(0, '.')
from ps2kit.config import cfg
from ps2kit.pine import Pine

P2S, PAD, SECONDS, SHOT = sys.argv[1], sys.argv[2], float(sys.argv[3]), sys.argv[4]
SLOT8 = '/home/rycs/repos/openmx-slots/s1-slots/pcsx2/s1/PCSX2/savestates/SLUS-20767 (9866FE54).08.p2s'
shutil.copy(P2S, SLOT8)
p = Pine()
p.load_state(8) if hasattr(p, 'load_state') else subprocess.run(['./kit', 'pine', 'load', '8'], check=True)
time.sleep(2.0)
GM = cfg().symbols['game_manager']


def rider():
    a = p.read32(GM)
    for off in (0x34, 0x34, 0x8, 0x8, 0x0):
        a = p.read32(a + off)
    return a


def tracks(r):
    """Each TireTrack entry of the rider's particle manager (rider +0x1490): its flags, its fade-out and
    fade-in distances left, and its strip's vertex count, ended flag, u, +0xd8 and ring position."""
    m = p.read32(r + 0x1490)
    n, arr = p.read32(m + 0x34), p.read32(m + 0x3c)
    out = []
    for k in range(n):
        e = p.read32(arr + 4 * k)
        if p.read32(e + 0x10) != 0x45e630:
            continue
        s = p.read32(e + 0x14)
        ring = p.read32(s + 0xf0)
        out.append(f"[fl{p.read8(e + 8)} out{p.read_f32(e + 0x44):.2f} in{p.read_f32(e + 0x4c):.2f} "
                   f"n{p.read32(s + 0x3c)} end{p.read32(s + 0x40)} u{p.read_f32(s + 0xd0):.2f} d8 {p.read_f32(s + 0xd8):.2f} "
                   f"a{p.read8(s + 0x1d4)} ring{p.read32(ring + 0x48)}/{p.read32(ring + 0x44)}.{p.read32(ring + 0x4c)}]")
    return ' '.join(out)


lines = [l.strip() for l in open(PAD) if l.strip() and not l.startswith('#')]
pad = subprocess.Popen(['./kit', 'pad', 'send', *lines])
t0 = time.time()
while time.time() - t0 < SECONDS:
    r = rider()
    f = lambda o: p.read_f32(r + o)
    i = lambda o: p.read32(r + o)
    zf, zr = p.read8(r + 0x910), p.read8(r + 0x911)
    pos = [f(0x8d0 + 4 * k) for k in range(3)]
    print(f"{time.time() - t0:5.1f} speed {f(0x860):6.2f} F z{zf} c{f(0x86c):.3f} g{i(0x914)} o{i(0x918)} s{i(0x91c)}"
          f"  R z{zr} c{f(0x87c):.3f} g{i(0x928)} o{i(0x92c)} s{i(0x930)}  {tracks(r)}",
          flush=True)
    time.sleep(0.25)
pad.wait()
subprocess.run(['./kit', 'shot', SHOT])
