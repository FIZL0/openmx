# P6 — riding overlay

2026-10-17. Task: `./kit plan P6`.

The shell can now show what the simulation thinks you pressed: `--hud` (H toggles it) draws a panel in
the bottom-left with the sixteen controls as the port's input manager holds them — held, pressed this
frame, and how hard for the four analogue ones — both sticks drawn twice over (where the report byte
is, and where `input.rs`'s dead zone put it), the three commands that ramp against what the pad asked
for, and the numbers the ride is running on: speed, rpm, gear, clutch, grounded, roll and pitch, the
state machine's phase, the last trick and the last wreck reason. It matches nothing on the disc and is
drawn without the rasteriser — a 2-D blit and a 3×5 font — at 47 µs a frame in release, against the
brief's budget of a millisecond, and nothing at all with the flag off. On its first frame it found a
real bug in the shell's own pad layer: with no joystick on the machine the clutch and the front brake
sat at 43% for ever, because a trigger axis was starting in the middle of its range instead of at its
rest. That fix is in the same commit.

Details in `1-riding-overlay-FINAL.md`.
