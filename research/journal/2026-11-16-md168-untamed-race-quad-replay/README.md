# MD168 — untamed race quad replay

2026-11-16. Task: `./kit plan MD168`.

I replayed Untamed's race quad from MD147's captures in crates/game-sim/tests/untamed_race_quad.rs.
- **Seed.** The quad is made from the untamed_atv pack's files, as the shell makes it, then its state is read from the spawn image at Untamed's moved offsets. Every key the files give equals the image's.
- **Ground.** Untamed's own ground query, run in the interpreter on the image's terrain.
- **Results.** steer.csv is bit-exact to the take-off (row 196). hazard-time.csv is bit-exact to the put-back tick (row 179), with Untamed's clock matching. hazard-on.csv is bit-exact to row 29.
- **Gaps.** The put-back itself differs: the original moves the quad a tick later and to its last free spot (filed as MD168a). steer's take-off gets a sideways knock the bare world here lacks (filed as MD168b).

Details in 1-untamed-race-quad-replay-FINAL.md.
