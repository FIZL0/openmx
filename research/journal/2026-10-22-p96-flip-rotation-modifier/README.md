# P96 — flip rotation modifier

2026-10-22. Task: `./kit plan P96`.

A "Flip rotation speed" modifier (`rotation_speed`, `Knob::Rotation`). It replaces P55's torque-only
"Flip spin" on the menu; `spin_rate` still parses. It is one factor, `air::RotationSpeed`, that every
rotation in the air reads: rotation rows through `Rotation::tuned` (the backflip, the port's flat spin
and MD16's front flip), MX vs. ATV's 360 through `SpinTuning::tuned`, the stick's air moments and
damping, and the chassis' angular windows. At 150% each of the five rotations turns in 0.66–0.67× the
ticks; at 100% nothing changes. Part 1 was the first attempt. Part 2 rebased it onto MD16, after a name
clash on `air::Spin` broke the merge.
