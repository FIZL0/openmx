"""F27a1: the port's run-up as a shell pad tape: full gas (Cross at full pressure) for GAS ticks from
the first, then nothing, TICKS ticks in all at the 30 Hz frame (memory: hand-written shell tape).

usage: python3 -I research/f27a1/tape.py GAS TICKS OUT.tape"""
import sys

gas, ticks, out = int(sys.argv[1]), int(sys.argv[2]), sys.argv[3]
idle = bytearray.fromhex('0079ffff7f7f7f7f') + bytes(24)
cross = bytearray(idle)
cross[3] = 0xbf                  # buttons active-low: Cross
cross[14] = 0xff                 # and its pressure
lines = ['# game-sim pad tape: %d frames' % ticks, 'format 1', 'tape f27a1-runup']
for t in range(ticks):
    lines.append('frame 0x3d08850a 0x%s -' % (cross if t < gas else idle).hex())
open(out, 'w').write('\n'.join(lines) + '\n')
