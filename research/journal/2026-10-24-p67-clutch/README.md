# P67 — clutch

2026-10-24. Task: `./kit plan P67`.

The original has a clutch: command 0x10, on L1 in the shipped preset, which the bike reads as a
switch. Pulled in, it lets the engine rev with the rear wheel free; let out, it launches on a torque
ramp (up to 2.5x at a standstill) with the centre of gravity moved back. The port already read it,
and P40's new layout already left it on L1, so this task proved it through the shell's remap, bit for
bit against the original layout. It also fixed the collision with the L1+R1 put-back: under the new
layout the chord's halves must go down within 250 ms of each other (`Controls::chord_together`), so a
clutch held into an R1 trick is no longer a reset. The original layout keeps the disc's chord.
Details in the numbered files; the newest one's suffix says whose move it is (FINAL · PART · READY).
