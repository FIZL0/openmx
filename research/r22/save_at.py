"""R22: save the running copy's state to SLOT once the game's frame counter (0x479d04) reaches FRAME.

    ./kit pcsx2 with -- python3 -B research/r22/save_at.py FRAME SLOT [TIMEOUT_S]

Run beside research/r22/play.sh (which plays the tape). The save lands a frame or two after FRAME
(PINE's save is queued); read the image's own counter after.
"""
import sys, time
sys.path.insert(0, '.')
from ps2kit.pine import Pine

frame, slot = int(sys.argv[1]), int(sys.argv[2])
end = time.time() + (float(sys.argv[3]) if len(sys.argv) > 3 else 120)
p = Pine(check=False, lock=False, timeout=5.0)
while time.time() < end:
    try:
        f = p.read32(0x00479d04)
    except (OSError, RuntimeError):
        f = 0
    if f >= frame:
        p.save_state(slot)
        print('saved', slot, 'at frame', f, flush=True)
        break
    time.sleep(0.002)
else:
    raise SystemExit('frame %d not reached' % frame)
