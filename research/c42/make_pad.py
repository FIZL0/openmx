"""C42: write trick.pad, the tape's script (research/c15/tape.py's language).

Gas (Cross) from the saved start of the sx01 race. From tick TRICK0 to TRICK1 a no-hander (Triangle with the
stick up, C8) is pressed for PRESS ticks every PERIOD ticks: only the presses that fall in the air start a
trick; one released before the landing banks into the card's +0xdc at the landing. Then gas only, lifted for
LIFT ticks every PULSE ticks to the end: each lift and press again is a Cross press, which the results
screen takes as accept, Restart (0x36). Triangle stops before the results, since it is the screen's Quit.

usage: make_pad.py > trick.pad
"""
TRICK0, TRICK1, PERIOD, PRESS = 300, 3300, 45, 8
END, PULSE, LIFT = 17000, 30, 3

out = ['# C42 (research/c42/make_pad.py): gas + no-hander presses, then Cross pulses for the results\' Restart',
       'down cross', f'sleep {TRICK0}t']
t = TRICK0
while t + PERIOD <= TRICK1:
    out += ['down triangle', 'stick l 0 -1', f'sleep {PRESS}t', 'up triangle', 'stick l 0 0', f'sleep {PERIOD - PRESS}t']
    t += PERIOD
while t + PULSE <= END:
    out += ['up cross', f'sleep {LIFT}t', 'down cross', f'sleep {PULSE - LIFT}t']
    t += PULSE
print('\n'.join(out))
