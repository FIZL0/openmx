"""Act at given ticks of a running playback (S93): save a state, or take a screenshot.

    python3 -B research/s93/at_ticks.py 3402:save:1 3400:shot:s93-long-3400 ...

Polls @frame_counter over PINE (the only client: run it with no recorder going) and, the first time
the counter reads a tick's value, does the action: `save:N` is PINE's save to slot N (the emulator
runs slow for this, KIT_SPEED / NominalScalar 0.25, so the save lands inside the tick it was asked in;
the RAM image's own frame_counter says which tick it holds), `shot:NAME` is `./kit shot
context/shots/NAME.png`. Exits once every action is done, or after 10 minutes.
"""
import os, subprocess, sys, time

sys.path.insert(0, os.getcwd())
from ps2kit.pine import Pine  # noqa: E402

FRAME_COUNTER = 0x00479d04
todo = sorted((int(t), kind, arg) for t, kind, arg in (a.split(':') for a in sys.argv[1:]))
pine = Pine()
end = time.time() + 600
while todo and time.time() < end:
    f = pine.read32(FRAME_COUNTER)
    while todo and f >= todo[0][0]:
        t, kind, arg = todo.pop(0)
        if f > t + 2:
            print('missed %d (counter at %d)' % (t, f), flush=True)
            continue
        if kind == 'save':
            pine.save_state(int(arg))
        else:
            subprocess.run(['./kit', 'shot', 'context/shots/%s.png' % arg], stdout=subprocess.DEVNULL)
        print('%s %s at counter %d (asked %d)' % (kind, arg, f, t), flush=True)
    time.sleep(0.002)
print('left undone: %s' % todo if todo else 'all done')
