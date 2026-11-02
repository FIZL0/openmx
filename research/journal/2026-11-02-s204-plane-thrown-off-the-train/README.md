# S204 — the plane thrown off the train

2026-11-02. Task: `./kit plan S204`.

S163 found that biplane.p2m2's second crash is bit-exact up to its wreck tick. On the next tick the
original's plane was thrown off something the port's free06 did not have. The something is free06's
train, which is crossing the hilltop as the plane skids onto it. With the save state's cars in the plane's
world, the port follows the original bit for bit on every tick from six before the wreck to the end of
both captures, 190 ticks past the wreck. That run takes in the strike on the train, the side of the next
car, the 150-tick wreck and the recovery onto the spawn. No sim code changed. Only
`tests/plane_crash_peak.rs` changed. Details in `1-plane-thrown-off-the-train-FINAL.md`.
