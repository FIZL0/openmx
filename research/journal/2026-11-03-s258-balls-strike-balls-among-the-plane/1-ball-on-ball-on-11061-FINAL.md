# 1 — ball on ball on hit0-11061 (FINAL)

Task S258. 2026-11-03.

## What I did

- Read S241's journal and the oracle's S241 and S242 blocks. S242 (landed after S241) ported a prop's
  rays against other props' `f.col` (`prop_props::strike_props`, `step_named`). `Sim::step`'s plane lane
  runs the props' search (`Traffic::frame_held`) before the plane's frame, and steps them after it with
  `step_with` → `step_named`. So the lane already had the strikes and the pushes, but they were not
  proved there. Nothing was taken from opentennis: it has no props.
- Copied s2's S241 states (`context/s241`, `context/states/s241-*`, `context/capture/s241`) into s1's
  context.
- New `tests/prop_plane.rs` helper `lane_11061`. It seeds free06's six balls from hit0-11061 (body,
  node, filed place, countdown, fresh, collider rest, sweep), woken as a strike wakes them. Then it runs
  the lane's order: `PropSet::search` (own searches, then the strikes on each other), the plane's
  `search_props` (it strikes no ball here, asserted), then `struck_on` on the hull at the plane's node.
- `the_balls_rays_strike_each_other_as_the_originals` compares this with the original's node
  re-searched: every ball's whole list in order (kind, what it names, from/on_body/normal/point,
  surface) and the four flags. It failed once. Ball 0 (filed 6th) had
  `[terrain x3, k1 ball 2, k0 plane]` where the original has `[terrain x3, k0 plane, k1 ball 2]`. Ball 2
  is filed after ball 0, so its mirror goes after ball 0's own entries, and the hull entry is one of
  those. Fix in `prop_wreck::struck_on`: walk the props in the node's order (`stamp_filed` +
  `node_order`, which S241 had left as collection order). New `own_end` inserts a prop's entries ahead
  of the trailing kind-1 mirrors that props filed after it left there. With no such mirror (every wreck
  bike case so far) it appends, as before.
- `the_balls_push_each_other_as_the_originals`: `NODE_UPDATE` run on hit0-11061 in the oracle, with
  the plane's body (and its record's cofg_world, normal speed, relative) taken when the first ball's
  resolve is called. On the port side, `PropSet::step` after `lane_11061`, with that plane as striker 0.
  All six balls' position, velocity, quat, rates, shock and twist match bit for bit, and so do the
  plane's velocity and rates after the tick. That covers ball 2's entry pushing ball 0 and itself, ball
  0's mirror, and balls 0's and 4's hull entries pushing the plane. Without the `struck_on` fix, both
  tests fail.
- The test's `seed` matched each RAM ball to the nearest prop node. That gave two balls one prop (11061:
  balls 3 and 5). Picking each prop only once still swapped knocked balls (11061: ball 0 stands nearest
  ball 2's spawn). Since the props are made in the original's collection order, it now maps by order.
  The new `seed_by_place` checks that order on hit0-11042, where every ball is still at its spawn. The
  original's node update resolves the balls in that same order. The four older S241 tests still pass.

## Checked

- `cargo test --release -p game-sim --test prop_plane -- --nocapture`: 6 passed, none skipped (the
  lists and pushes are printed).
- With s3's s195/s242/s197b/s244 states copied in, nothing skips and all pass: prop_wreck 5, prop_props 3,
  prop_knock 9.
- `./kit check -- -p game-sim --no-fail-fast`: everything else is green. Two reds, both already known:
  - fly_and_collide's `the_drawn_rider_leans_as_the_original_through_the_turns` gets (1952, 282)
    against 280. It fails the same with this change backed out (B18).
  - `the_humans_tricks` is known red on C6's capture.

## Not verified / not 1:1

- The order between a prop's own hull entries and its own ball-on-ball kind-0 entries: no ball in
  hit0-11061 has both. The port puts the ball entries first (strike_props runs in the props' search,
  before the plane's frame). The original makes one narrow pass, ordered by the kept nearby list.
- A ray that would meet both the hull and another ball keeps both in the port (two passes) and one, the
  nearer, in the original's single record. That does not happen in hit0-11061.
- One tick only (11061 to its node update), from the state. No multi-tick replay of the hit0 balls: the
  plane's own state (tires, surfaces) is not seeded beyond its body.
- The plane's mirrors on a ball (its rays striking it) and ball-on-ball mirrors in the same tick: none
  in 11061. `PropSet::strike` puts the plane's at the head, which is the original's rule for a
  first-filed striker. Not seen together.
- The wreck bike pass (`struck_by_props`) now also walks node order. That order is not proved on
  a wreck state with props filed out of collection order. The tests for it pass.
