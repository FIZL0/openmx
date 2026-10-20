"""S37: while recording2 plays back, save a state on each of the ticks just before the reason-6 wreck.

Polls frame_counter over PINE (no next_frame: its stall watchdog dies during the state load) and asks
PCSX2 to save slot S when the counter first reads F, for each F:S pair given. Which tick each state
actually landed on is read off its RAM afterwards, not trusted from here.

    python3 save_before.py 4122:8 4123:9
"""
import sys
import time

sys.path.insert(0, '.')
from ps2kit.config import cfg  # noqa: E402
from ps2kit.pine import Pine  # noqa: E402

want = sorted((int(a), int(b)) for a, b in (x.split(':') for x in sys.argv[1:]))
p = Pine()
fc = cfg().sym('@frame_counter')
deadline = time.time() + 240
while want and time.time() < deadline:
    f = p.read32(fc)
    if 4000 <= f and f >= want[0][0]:
        at, slot = want.pop(0)
        p.save_state(slot)
        print(f'asked for slot {slot} at frame_counter {f} (wanted {at})', flush=True)
    time.sleep(0.002)
print('left:', want, flush=True)
