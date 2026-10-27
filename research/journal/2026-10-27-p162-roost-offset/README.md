# P162 — roost offset

2026-10-27. Task: `./kit plan P162`.

The bike's kicked-up dirt came off a little away from the tire. In the bike's own frame the port's spray node was already where the original's is: it sits 0.020 ft off P74's capture at the launch. The offset came from two things in the shell. First, the particles were stepped before the chassis was posed, so the nodes were a tick behind. Second, they were drawn at the last tick while the bike is drawn blended between ticks. Both are fixed. `roost_offset.rs` holds the launch point against the capture and the drawing against the blend. Details in 1-roost-offset-FINAL.md.
