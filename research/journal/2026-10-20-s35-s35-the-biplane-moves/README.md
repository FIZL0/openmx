# S35 — s35 the biplane moves

2026-10-20. Task: `./kit plan S35`.

The biplane's pilot and moving parts, driven from the flight state (`crates/game-sim/src/biplane_anim.rs`).
The pilot is a rider's body playing one added record, `biplane_roll_bck`/`biplane_roll_fwd`, off two lags
that the plane's request block steers from the sticks. That chain is proved bit for bit on every tick of the
human's biplane flight, re-captured for the pilot's fields (`watch/biplane-pilot.toml`). The flaps, rudder
and prop (with the bent cone once wrecked) match the original's bone matrices to tolerance. The plane object
that would call all this does not exist in the port yet; the wiring list is in file 1.
