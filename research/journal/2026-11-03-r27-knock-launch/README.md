# R27 — knock launch

2026-11-03. Task: `./kit plan R27`.

The shell's struck free09 beach ball rose slower than the original's (F27a1). The port's whole ride
was seeded off f27a1's RAM images and stepped against the next image and the capture
(`crates/game-sim/tests/knock_launch.rs`). The launch's second and third ticks of contact are bit for
bit. The first one after the strike is not. The original's ball also files its own probe's contact
on the bike's frame, which throws it up (vy 19.31). The port only hears that hit (F27a2) and files
no contact (vy 13.88). Filed as B29. Seeded from the original's state, the port's bike never strikes
the ball again in 60 ticks, so F27a1's second bbhit is not reproduced from the original's state.
Details in `1-knock-launch-FINAL.md`.

## Files

| file | what |
|---|---|
| `crates/game-sim/tests/knock_launch.rs` | the seeded ride: two ticks bit for bit, B29's ignored test, 60 ticks against the csv |
