"""S149a: from a boot.py state, switch the human into the parked car through the pause menu and save.

usage: switch.py SLOT OUT SHOT

Loads SLOT, sets the cheat flag the menu's machine list checks (read32(0x004e2778)+0x5c, S148/P103),
pauses (Start), goes down twice (the greyed Restart Activity and Activity Instructions are skipped, so
that lands on "Switch To Machine: <car>"), presses Cross, writes SHOT with the car's controls card up,
accepts the card with Cross, lets one second run and saves slot OUT. The car stands where the boot
parked it, the human in it.
"""
import subprocess, sys, time
sys.path.insert(0, '.')
from ps2kit.pine import Pine

slot, out, shot = int(sys.argv[1]), int(sys.argv[2]), sys.argv[3]
p = Pine(check=False, lock=False, timeout=5.0)
p.load_state(slot)
time.sleep(3)
p.write32(p.read32(0x004e2778) + 0x5c, 1)


def pad(*c):
    """Send the commands and wait them out: `./kit pad send` returns before its sleeps play."""
    subprocess.run(['./kit', 'pad', 'send', *c], check=True)
    time.sleep(sum(int(x.split()[-1]) for x in c if x.split()[0] in ('press', 'sleep')) / 1000 + 0.3)


pad('release', 'press start 120', 'sleep 1500', 'press down 120', 'sleep 500', 'press down 120', 'sleep 500',
    'press cross 120', 'sleep 3000')
subprocess.run(['./kit', 'shot', shot], check=True, capture_output=True)
pad('press cross 150', 'sleep 1000')
p.save_state(out)
time.sleep(2)
print('switched, saved', out, flush=True)
