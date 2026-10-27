# S115 — nose first touchdown

2026-10-27. Task: `./kit plan S115`.

S67's nose-first drop (touchdown 3459) parted from the original by 2.69° and 2.36 in the velocity. A new
playback saved states at 3457 and 3458. On main, the drop already rode within S67_BOUND, closed by some
change since S67 (not bisected). Running the original's vehicle update from the 3458 image beside the
port's tick left one ulp in the centre of gravity: the transform's point call adds the origin first, and
the port added it last. `ride.rs`'s `orientation_point` now answers the integrator's PointCofg. The
touchdown tick is bit for bit on 16 lanes, S67_NOSE_FIRST_BOUND is gone, and the hash pins moved by
that ulp are re-pinned. Details in 1-nose-first-touchdown-FINAL.md.
