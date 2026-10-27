"""Save a state at each given tick of a running 2p playback (C10): S93's at_ticks.py, one slot per tick.

    python3 -B research/c10/at_ticks.py SLOT0 TICK...

Polls @frame_counter over PINE (the only client: no recorder running) and, the first time the counter
reads a tick, saves to the next slot from SLOT0 on (the emulator runs at NominalScalar 0.05 so the save
lands inside the tick asked for; the RAM image's own frame_counter says which tick it holds). Prints
`tick slot` per save, for research/c10/rams.sh. Exits when every tick is done, or after 100 minutes
(the whole ride at NominalScalar 0.05).
"""
import os
import sys
import time

sys.path.insert(0, os.getcwd())
from ps2kit.pine import Pine  # noqa: E402

FRAME_COUNTER = 0x00479d04
slot = int(sys.argv[1])
todo = sorted(set(int(t) for t in sys.argv[2:]))
pine = Pine()
end = time.time() + 6000
while todo and time.time() < end:
    f = pine.read32(FRAME_COUNTER)
    while todo and f >= todo[0]:
        t = todo.pop(0)
        if f > t + 2:
            print('missed %d (counter at %d)' % (t, f), flush=True)
            continue
        pine.save_state(slot)
        print('%d %d at %d' % (t, slot, f), flush=True)
        slot += 1
    time.sleep(0.002)
print('left undone: %s' % todo if todo else 'all done')
