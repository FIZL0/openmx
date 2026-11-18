# S437 — v40 tape from the state

2026-11-17. Task: `./kit plan S437`.

V120 saw the shell's ride of V40's tape part from the original's: the brake to a dead stop, a launch spin
of 35 ticks. The shell doesn't ride first_recording's bike. It starts a fresh rider at the start line, and
that rider's clutch launch ramp is unspent, so it has 3× torque at first. Recorded the original on the
tape from first_recording's state (`research/s437/capture.sh`) and replayed it in the port from the same
state: all 112 lanes agree on all 293 ticks, the launch spin, the brake to ~7, the roll on to ~14 and the
spin there included. No sim code changed; `replay.rs` pins it. Details in `1-v40-tape-from-the-state-FINAL.md`.
