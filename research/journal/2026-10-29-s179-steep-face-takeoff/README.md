# S179 — steep face takeoff

2026-10-29. Task: `./kit plan S179`.

S117's held traverses of a steep face parted on the first airborne ticks. On current main the big gap was
already gone; what was left was an ulp in the roll lane of the angular acceleration a tick or two into
the flight. Seven RAM images of a new held ride, each run one tick through the original's vehicle update
in the oracle, show the cause. The original measures the chassis' euler angles once a frame, after the
physics. The port took them again at the top of each frame, off the quaternion the stunt clocks' copy-back
had moved by an ulp. With that gone, the takeoff and flight agree to the bit on all seven images. S58's
turn, launch, brake, traverse and steep agree to their captures' last tick, and steep2's path to 188 (was
106). Details in 1.
