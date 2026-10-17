# V17 — the frames the port cannot step

2026-10-17. Task: `./kit plan V17`.

V15's stepping test skipped two shapes of tick, each with its shape asserted. The first — the frame a
clip-to-clip transition **ends** on — was a real port bug: the renderer had no transition sampler at
all, and the weight was thrown away by the advance's own end-of-frame reset before anything could
draw it. `motion::Cross` now carries it out of the advance, `skin::Clip::pose_crossed` samples both
of the original's arms (the outgoing clip, or the pose snapshot) and `anim::Set::apply_vehicle_crossed`
dispatches it on mode 0, where the original's dispatch leaves it visible. The second — the frame a
**wreck is declared** on, where the vehicle supposedly skipped the rider's animation step and the next
frame latched twice — is refuted by the recording that produced it: that row is an early read, the
recording's *other* wreck is read after the step and steps normally, and the "double latch" is two
frames latching once each. Both skips are gone; 1393 of 1499 pairs are now posed and compared, 24 of
them with a transition running, worst lane 5.3e-5. `crates/game-sim/src/vehicle_anim.rs` needed no
behavioural change.

Details in the numbered files; the newest one's suffix says whose move it is (FINAL · PART · READY).
