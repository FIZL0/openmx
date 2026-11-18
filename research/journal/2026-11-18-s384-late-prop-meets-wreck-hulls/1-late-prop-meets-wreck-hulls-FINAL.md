# 1 — late prop meets wreck hulls (FINAL)

Task S384. 2026-11-18.

## What I did

- `PropSet::strike_in_among` (prop_contact.rs): `strike_in` with the downed riders' wreck hulls. A prop
  the strike files searches with them in its world (`search_late`), and so do the props its rays wake
  (`strike_props_late`). It answers what those props' rays struck on each wreck bike. `strike_in` is
  now `strike_in_among` with no hulls.
- `Sim::wreck_hulls` (prop_wreck.rs): the hull list `frame_held_wrecked` builds, split out so the
  two-rider step can keep it. `frame_held_wrecked` uses it.
- `prop_wreck::late_struck`: puts a late strike after the pass's on a wreck bike. Mirrors go after the
  pass's mirrors. Each names its body by its place in the bike's struck list (an existing body keeps its
  place, a new one goes at the end).
- `Sim::step_all` (lib.rs) keeps the hulls as the props' pass had them, and each bike's strike goes
  through `Sim::strike_into_among`. The mirrors are merged onto each downed rider's
  `crash.bike.struck` if that rider's frame has not run yet this tick (see below).
- Tests in `tests/prop_wreck.rs`: `a_ball_a_strike_wakes_meets_the_wreck_bike_beside_it`,
  `a_late_strike_goes_after_the_passs`.

## What the original does

From B34b's journal: the props node's collide walk re-reads its filed count at every step, so an object
a strike files is searched at the walk's end. The wreck bikes' objects are on the same node (S243). So
that late search has the wreck hulls in its world, like any filed prop's search. Its mirrors go on the
wreck bike's list after the ones from earlier in the walk.

## How it was proved

Port against port only. The hit2/hit3/hit5 RAM images (S197b, S244) are no longer anywhere in the shared
context (`find -L context -iname '*hit[235]-28*'` finds nothing). Without them there is no image with a
wreck bike on the props node to build the B34b-style interpreter scene from.

- `a_ball_a_strike_wakes_meets_the_wreck_bike_beside_it`: free09 off the disc, its asleep ball 2, a
  wreck bike hull (wreckbikef.col) at 1.5, 2.5, 3.5 and 4.5 ft along x. The ball is struck by a riding
  bike's strike and searched late (`strike_in_among`). Its list and its mirrors and bodies for the wreck
  bike equal those it gets when woken first and searched in the pass (`search_in_among`, the path S243
  proved against hit2). At every gap the list has one entry on the wreck. With no hulls (the old
  behaviour) it has none.
- `a_late_strike_goes_after_the_passs`: the merge's order and its renaming.
- Passing: prop_wreck (9), prop_plane, prop_props, knock_launch, prop_strike_paths, two_p_rider.

## Not verified / not 1:1

- **No comparison with the original.** The hit states that S243/S244 proved against are gone from the
  context, and no capture has one rider down beside a sleeping ball that another strikes. The test only
  shows that the late search matches the in-pass search, which S243 proved.
- **Wreck bikes already stepped this tick lose the late mirrors.** `step_all` walks riders backwards. A
  downed rider after the striker in player order has already run its frame (and taken its struck list)
  when the strike comes. The prop's entry on that wreck bike is still filed this tick, but the wreck
  bike's mirrors are dropped. In the original all strikes are in the props pass, before any frame.
  Fixing it would mean moving the riding bikes' strikes ahead of the rider walk (the S197a design).
- **The late struck bodies are copies from the strike.** The wreck bike's frame pushes them and writes
  them back after the striker's frame. That may undo the striker's push on the same ball. This is the
  same kind of stale copy the pass's struck bodies already have. Not checked against the original.
- **The plane lane is unchanged.** Its props pass (`frame_held`) puts no wreck hulls on the node at all
  (S276 meets downed bikes separately), so the plane's late search stays without them, to match that pass.
- **One-player `step` and `step_bumped` are unchanged.** Their only wreck is the striking player's own,
  and a downed rider does not strike.
- **The `Sim::step_all` wiring itself has no test.** It needs a two-rider Sim with one rider's ragdoll
  kit down beside a sleeping prop. Only the PropSet level and the merge are tested.
- The ball's list in the test has only the wreck entry (no ground entry at its spawn height); the test
  does not cover a late prop with terrain entries too.

## Next

None: the task lands.
