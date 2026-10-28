"""V70: the original's tire marks after a hard launch and a braked stop, from the chase camera, and
the pose the port is shot from.

Loads first_recording's state (free09) into copy 2's slot 8, plays research/v70/launch.pad, then
reads each TireTrack entry (as research/v40_tracks.py does), the render camera and its projection
(research/shots/moment.py's 0x0046a540 / 0x0046a580, read live), prints the camera as `--shot-pose`
numbers, and takes `./kit shot`. With NOTRACKS=1 it first sets both entries' MinimumSpeedThreshold
(entry +0x50, vehicle_fx_track_ctor's 10) to 1e9, so the same ride lays no marks: the tracks touch
nothing in the simulation. SHOT_WITHOUT: a second shot of the same frame with the rings emptied.

usage: python3 -B research/v70/shot.py SHOT PID [SHOT_WITHOUT] (under `./kit pcsx2 with --`, copy 2; PID resumes a paused VM)
"""
import math, os, shutil, struct, subprocess, sys, time
sys.path.insert(0, '.')
from ps2kit.config import cfg
from ps2kit.pine import Pine

SHOT, PID = sys.argv[1], sys.argv[2]
STATE = 'context/recordings/first_recording.p2m2_SaveState.p2s'
SLOT8 = '/home/rycs/repos/openmx-slots/s2-slots/pcsx2/s2/PCSX2/savestates/SLUS-20767 (9866FE54).08.p2s'
PAD = 'research/v70/launch.pad'
RENDER_CAMERA, VIEW_PROJECTION = 0x0046A540, 0x0046A580

subprocess.run(['python3', '-B', 'research/journal/2026-10-20-c6-c6-recording2-capture/playback/resume_if_paused.py', PID])
shutil.copy(STATE, SLOT8)
subprocess.run(['./kit', 'pine', 'load', '8'], check=True)
time.sleep(2.0)
p = Pine()
GM = cfg().symbols['game_manager']


def rider():
    a = p.read32(GM)
    for off in (0x34, 0x34, 0x8, 0x8, 0x0):
        a = p.read32(a + off)
    return a


def tracks(r):
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
                   f"n{p.read32(s + 0x3c)} end{p.read32(s + 0x40)} a{p.read8(s + 0x1d4)} "
                   f"ring{p.read32(ring + 0x48)}/{p.read32(ring + 0x44)}.{p.read32(ring + 0x4c)}]")
    return ' '.join(out)


def line(t0):
    r = rider()
    f = lambda o: p.read_f32(r + o)
    i = lambda o: p.read32(r + o)
    pos = [round(f(0x8f0 + 4 * k), 1) for k in range(3)]
    print(f"{time.time() - t0:5.1f} speed {f(0x860):6.2f} rear {pos} F z{p.read8(r + 0x910)} c{f(0x86c):.3f} "
          f"s{i(0x91c)}  R z{p.read8(r + 0x911)} c{f(0x87c):.3f} s{i(0x930)}  {tracks(r)}", flush=True)


if os.environ.get('NOTRACKS') == '1':
    m = p.read32(rider() + 0x1490)
    for k in range(p.read32(m + 0x34)):
        e = p.read32(p.read32(m + 0x3c) + 4 * k)
        if p.read32(e + 0x10) == 0x45e630:
            p.write32(e + 0x50, struct.unpack('<I', struct.pack('<f', 1e9))[0])
            print(f"entry {k}: no marks")

lines = [l.strip() for l in open(PAD) if l.strip() and not l.startswith('#')]
total = sum(int(l.split()[1]) for l in lines if l.startswith('sleep')) / 1000.0
t0 = time.time()
subprocess.run(['./kit', 'pad', 'send', *lines], check=True)
while time.time() - t0 < total + 0.5:
    line(t0)
    time.sleep(0.25)

eye = [p.read_f32(RENDER_CAMERA + 0x30 + 4 * k) for k in range(3)]
vp = [[p.read_f32(VIEW_PROJECTION + 16 * r + 4 * c) for c in range(4)] for r in range(4)]
col = lambda c: [vp[r][c] for r in range(3)]
norm = lambda v: math.sqrt(sum(x * x for x in v))
fwd = [x / norm(col(3)) for x in col(3)]
fov_h = 2 * math.degrees(math.atan(1.0 / norm(col(0))))
yaw, pitch = math.degrees(math.atan2(fwd[0], fwd[2])), math.degrees(math.asin(fwd[1]))
subprocess.run(['./kit', 'shot', SHOT], check=True)
if len(sys.argv) > 3:
    # The same frame without the marks: each entry's ring told it has no batches (ring +0x44), with
    # the bike and the camera standing still, and a second shot.
    m = p.read32(rider() + 0x1490)
    for k in range(p.read32(m + 0x34)):
        e = p.read32(p.read32(m + 0x3c) + 4 * k)
        if p.read32(e + 0x10) == 0x45e630:
            p.write32(p.read32(p.read32(e + 0x14) + 0xf0) + 0x44, 0)
    time.sleep(0.15)
    subprocess.run(['./kit', 'shot', sys.argv[3]], check=True)
subprocess.run(['./kit', 'pad', 'send', 'release'], check=True)
eye2 = [p.read_f32(RENDER_CAMERA + 0x30 + 4 * k) for k in range(3)]
print(f"shot-pose {eye[0]:.3f},{eye[1]:.3f},{eye[2]:.3f},{yaw:.3f},{pitch:.3f},{fov_h:.3f}")
print(f"camera moved {norm([a - b for a, b in zip(eye, eye2)]):.4f} ft across the shot")
