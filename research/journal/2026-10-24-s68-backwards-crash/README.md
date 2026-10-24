# S68 — backwards crash

2026-10-24. Task: `./kit plan S68`.

The user reported (10-21) that a bike riding backwards can turn hard into the ground and never crash. I
recorded two backwards full-lock turns in the original from the recording's save state, one on free09's
flat and one reversing down its steep slope. The original never wrecks in either: its wreck test does not
read the direction, and its ground roll command is 0 below speed 0, so the reversing bike stays upright.
The port's wreck test and roll command are the same. Replayed from the same state, the flat run agrees on
every tick, the slope run agrees until its forward turn parts, and neither wrecks. No port fix was
needed. `the_captured_backwards_turns_never_wreck` in tests/replay.rs now holds it. The port has no pad
reverse yet (P72/P77). Details in 1-backwards-crash.
