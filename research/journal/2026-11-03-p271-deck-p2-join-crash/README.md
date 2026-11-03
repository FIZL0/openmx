# P271 — the Deck crashed when player 2 joined

2026-11-03. Task: `./kit plan P271`.

A second pad joining a running ride took the shell down whenever the join's tick went to the sim
thread (N23a). That is every tick under `--sim-thread on`, and under the default `auto` once two
seats ride and a tick plus a picture overrun the picture's time, which is a Deck. That tick's second
half, which lays out each view's rider and camera, waits for the picture after it, so the picture
drew two views from the one rider of the tick before: `index out of bounds: the len is 1 but the
index is 1` at `RideHud::with`'s `self.ridden[v]`. Reproduced on the desktop release build with two
stand-in pads. Fixed by keeping a reseating tick on the loop.

Two more things are fixed on the way. A window that reports no size split a 1x1 frame into views
0 px wide, which panicked the rasteriser and the sky. A joiner whose seat can't be built now leaves a
note and the ride goes on, instead of the error ending the ride.

Test: `crates/openmx/tests/drop_in.rs` (4 cases: mid-ride and over the pause, sim thread on and off).
Driver for hand runs: `pads.py`.
