# 1 — the ball's rays strike the wreck bike (FINAL)

Task S197b. 2026-11-02.

## What I did

- Capture (PCSX2 copy 2, research/s197b scripts, states in s2's `context/states/s197b-hit2-*`):
  `drop.py 3837316 1 2 10 30 25 150 2` wrecks the bike on free09 (state drop2-2804), `put_ball.py PID 3 2 4
  0.3 2` puts ball 2 in the tumbling wreck's way (relative to the wreck: a state load and a 3 s wait run
  ~22 frames, so absolute spots miss), `wreck.py hit2 2 2 --frames 90 --near 5` saves a state a frame
  around the hit, `slots.py ram` lists the props node's filed records.
- Found (oracle S197b block): the wreck's collision object is filed on the props node ahead of the balls;
  the ball's rays hit the wreck's f.col (its asleep probe categories, the wake mask 0x40, are in the
  wreck's categories); the wreck's rays never hit a ball.
- `prop_wreck::struck_by_props`: every filed prop with a ray tree searches the wreck's mesh
  (`world_search::search_nearby`, two collidables: the prop's probe and the wreck) and files each hit as
  the original does: a kind-0 entry on the prop's list naming the wreck (`me`, after the players'
  vehicles), a kind-1 mirror kept for the wreck's list, the prop woken. `WreckBike.struck` carries the
  mirrors and the pushed props into the wreck's tick (`tick_struck`: mirrors after its own entries,
  `step_among` with the props as others), not on the wire.
- `Sim::wreck_strike(i)` in `Sim::step`, `step_all`, `step_bumped`, `step_among`, after the bike's
  strike; `props_step` steps the props with the wreck bikes as strikers after the players' vehicles and
  writes their pushes back (`wreck_sides` / `wrecks_pushed`).
- tests/prop_wreck.rs `the_ball_strikes_the_wreck_bike` (states + disc): for hit2-2828, 2829, 2830 the
  ball's list (8, 5, 2 entries), the wreck's list (6, 4, 4 with 4, 2, 2 mirrors) and both bodies after
  the tick are the original's bit for bit. With the mirrors dropped and the entries made kind 1 it fails.

## Not verified / not 1:1

- The ball's impulse on the wreck bike did not move the wreck's body in these three ticks (it matches
  either way bar the lists), so the push on the wreck is not proved; the push on the ball is.
- Only one striker's mesh per search: the scene's hits and the wreck's are separate searches, so the
  original's one-entry-per-ray across both is not kept; in hit2 they never fall on one ray.
- Every filed prop searches the wreck (S197's stand-in), not the original's broad-phase gather; the
  wreck's sweep probe is not ported.
- Not done (filed as follow-ups): the rider/ragdoll and the plane striking props, props by props, the
  props' rays on the riding bike's f.col, the broad-phase gather, a Sim-level test of the wiring.
