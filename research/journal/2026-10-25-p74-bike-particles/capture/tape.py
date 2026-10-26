"""P74: write a shell tape that rides the port through a full-gas launch, a hard turn and a stop.

usage: python3 -B tape.py > launch.tape

One line a 30 Hz ride tick. 30 ticks idle, 150 on full gas (Cross), 60 on gas with the stick hard
left, 90 off everything.
"""

IDLE = bytearray.fromhex('0079ffff7f7f7f7f') + bytearray(24)


def report(gas=False, left=False):
    r = bytearray(IDLE)
    if gas:
        r[3] = 0xbf
        r[14] = 0xff
    if left:
        r[6] = 0x00
    return r.hex()


ticks = [report()] * 30 + [report(gas=True)] * 150 + [report(gas=True, left=True)] * 60 + [report()] * 90
print(f'# game-sim pad tape: {len(ticks)} frames')
print('format 1')
print('tape p74-launch')
for t in ticks:
    print(f'frame 0x3d088889 0x{t} -')
