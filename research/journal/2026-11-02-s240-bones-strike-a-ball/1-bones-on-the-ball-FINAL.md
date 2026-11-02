# 1 — the bones strike the ball (FINAL)

Task S240. 2026-11-02.

## What I did

- Capture (PCSX2 copy 3, research/s240 scripts, states in s3's `context/states/s240-drop2-*` and
  `context/s240/*.p2s`): load s197b's hit2-2828, `put_ball.py PID 1 1 0 3.0 2` (ball 1 three feet above
  bone 2's body), `strike.py drop2 2 --frames 30 --at 2840,...,2845` records watch/s240-ragdoll.toml
  (`context/capture/s240/drop2.csv`; drop1 the same run, identical) and saves the states. A paused game
  stalls the recorder: resume it first (c6's `resume_if_paused.py`).
- Found (oracle S240 block): bone 2's collision is on ball 1's mesh collidable at 2840 and 2841. The
  collidable is category 0x18, flags 0x14 (awake: the moving structure, listed wherever the walk meets
  it), its `+0x80` the ball's collision object; its listeners' "struck" slots are the object's wake and
  the collection's activate (`PropSet::struck`). The ball's contact list has only terrain entries with a
  bone in its ray reach: the props' rays do not meet the bones, so that half of the brief does not exist
  in the original.
- `ragdoll_scene::KnockedProp`: an awake prop's `f.col` where its node stands it, its distance from the
  bones' probe (`reach_distance`), its body and collision object copied (`prop_contact::Pushed`).
  `WreckBikeScene::knocked` offers them to each sphere on its one record after the statics and the
  bike, before the cars; a contact's other is the prop (`Against::Other`, `Side::Body`), and a prop that
  took a record is marked struck.
- `Down::step_posed_among` (`step_posed_with` calls it with none) lists them once the search is built and
  strikes the marked ones after the step; rider.rs's down branch passes the props lent to the ride for
  its frame (`Ride::props`, lent on every Sim step path).
- tests/ragdoll_ball.rs (states + disc + kit):
  `the_bones_search_lists_the_awake_balls_as_the_originals` (500 gathers, 3 balls each, distances bit
  for bit), `the_sphere_test_on_a_ball_answers_as_the_originals` (20,000 spheres, 11,322 hits),
  `bone_2_strikes_the_ball_as_in_the_capture` (2841's point and normal bit for bit).

## Not verified / not 1:1

- No whole tick is compared: the ragdoll's step from drop2-2840 against 2841 (the bones after the
  push-out) is not run; the push-out against an `Against::Other` body is tests/ragdoll_contact.rs's proof
  on generated bodies, not this state's.
- The struck listener runs after the bones' step, not at the hit. Only awake balls are listed, for which
  the wake and the activate do nothing, so this shows nowhere yet; a ball at rest is in the static tree,
  listed only inside the bound, and is F7's.
- The order among the moving entries: the original's list in drop2 has the balls ahead of the wreck bike;
  the port offers them after the statics and the bike. It matters only for two hits at the same plane
  distance, and for a list at its 128 capacity.
- A listed prop is every awake one, as the original's moving structure; the port does not model the
  other riders' bikes and the hierarchy meshes that share the list.
- The ball's push on the bone is from the ball's body as the props' last step left it; the props' step
  runs after the rider's frame in the port as in the original, which 2841's contact agreeing supports.
