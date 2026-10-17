# V18 — v18 the pose the shell draws

2026-10-17. Task: `./kit plan V18`.

The rider was posed by the wrong machine. V8 drew the body out of the bike's 28-slot `motion::Driver`
because that was the only machine the port had; V13 then ported the body's own sixty-state one
(`game_sim::rider_anim` in front of `game_sim::vehicle_anim`) and nothing but its own test ever called
it — which the original settles, since the selection at `./kit fn 0x00248a70` has no call site at all,
only two actor vtables. So the two are swapped over: the renderer binds the sixty-state table over the
set of clips it already loaded (`Body::pose_lib`, no second `Set`), `Body::ride` runs the body's step
and poses the skeleton from its frame, and the bike keeps its own driver. Two pieces the step was
throwing away had to come out with the frame — V17's transition and the put's snapshot answer — which
is the new `rider_anim::Clocked` and `Rider::body_clocked`; and the sim's body turned out to build its
two eased axes with a zero tau, which `Lag::step` divides by, so a foot sat at `-FLT_MAX` until V12's
`MIX_TAU`/`PHASE_TAU`/`AXIS_START` moved into `vehicle_anim` and into `Body::default`. A frame of the
shell's own driving now poses the body with every hand and foot anchor within 0.154 ft of the
chassis's (the tolerance is 1.5, the bind pose is 5.050).

Details in the numbered files; the newest one's suffix says whose move it is (FINAL · PART · READY).
