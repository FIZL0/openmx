# 1 — the wheel probe on the props (FINAL)

Task S210. 2026-11-02.

## What I did

- **New module `crates/game-sim/src/prop_wheel.rs`.**
  - `search` runs the wheel probe against every prop with a collidable mesh, posed where its node
    stands it (as `prop_contact::strike` poses them for the body rays). It works on the wheel record
    the scene's search left, so one contact a segment is kept and the nearer one wins (the record's
    own rule). It wakes every prop it strikes (`PropSet::struck`).
  - `handle` / `named` give the port's owner word for a prop: 1 + its place in the set, so never 0.
  - `under` reads what the wheel stands on: `PropSet::under`, or the bump's pushed copy when the body
    rays struck that prop this tick.
  - `Sim::lend_props` / `Sim::props_back` move the set into the ride for the bike's frame and back
    after it.
- **`ride.rs`.**
  - New field `Ride::props`, `None` between frames. The hash walk and the snapshot skip it, so the
    state hash is unchanged.
  - `wheel_hits` adds the props' contacts with their owner word and material. On a world that answers
    only `World::solid` (the tests' `Flat`), it merges them a segment at a time, nearer wins.
  - `contacts` writes the owner into the distributor's contact.
  - `under` asks the owner (`+0x1f0`) before the vehicle, as the original does. `share_load` passes
    it on.
- **The four step paths lend the props around the bike's frame:** `Sim::step` (both branches),
  `step_all` (each player), `step_bumped` and `step_among`. The lend comes before the bump's
  `pushed()` write-back.
- `snapshot.rs`: `props` added to `Ride`'s skip list. `tests/ride.rs`: the ride's size goes from
  2464 to 2472 (one box pointer).
- `research/oracle.txt`: an S210 block under S208's. No new names.
- Nothing taken from opentennis: it has no counterpart.

## What the original does

- In each of S195's knock states (2881/2882/2883/2897), every free09 prop's `f.col` (62 of them) has:
  category 0x18, the nearby bit set (asleep and awake), no mover, and the owner = the object's collision
  object. `collision_pair_filter` accepts the bike's wheel probe against it with nearby 0 and with 1. So
  the wheel stands on any knockable prop, at rest or moving.
- None of the saved states has a wheel on a prop: the wheel record is empty in all thirteen s195
  states. So the proof is an interpreter run on knock-2897:
  - stand the bike's node so a wheel segment passes near a prop (all 62 props, 24 placings each,
    half with the state's turn and half with none);
  - list all 62 meshes in the bike's nearby search;
  - call `probe_object_moved`, then `collide_against_search` on the wheel probe.
- **All 1,488 placings match `prop_wheel::search` bit for bit:** the record's count, mesh, point,
  normal and fraction. 290 placings have a contact, 15 on both segments.
- **The wake matches too.** After each placing, every body's enabled word and every collection's active
  word match the port's. 40 of the 60 sleeping props were woken along the way.
- The wheel's owner branch (`contact::other_at`, body side) was already proved bit for bit by S169.

## How it was checked

- `cargo test --release -p game-sim --test prop_knock`: 8 passed. New tests:
  - `the_wheel_probe_takes_every_props_mesh` (248 meshes over 4 states);
  - `the_wheel_probe_stands_on_the_props_as_the_originals` (the 1,488 placings).
- `--test prop_strike_paths`: new test `a_wheel_stands_on_the_ball_in_every_path`. Over free09's
  collection 5, in `step`, `step_all` (either player), `step_bumped` and `step_among`, it checks:
  - a wheel's owner word is the prop's handle, and the hit wakes the prop;
  - with no props, the same spot gives owner 0;
  - with the prop rolling at 8 ft/s under the wheel instead of still, the bike's velocity after the
    tick differs;
  - the props are back in the traffic after the frame.
- The suites the change touches, run with no skips (the disc and s195 states are in this slot):
  replay 109 passed, ride, bike_col, prop_*, head_on_props, ragdoll_props, event_ride. Then
  `./kit check -- -p game-sim`.

## Not verified / not 1:1

- **Whether a wheel on a prop pushes it.** The prop's motion reaches the bike. Whether the original's
  tire or suspension forces on a contact with an owner push the owner's body was not read. Filed as R14.
- **The gate.** The port measures each prop's distance from the probe's reach at the substep pose. The
  original gates on the bike's nearby list, gathered once a tick, with scene and props in the list's
  order; the port searches the props after the scene. A prop coming into reach mid-frame, or an exact
  tie on one segment between a scene contact and a prop's, can differ. Filed as F8.
- **No whole ride was compared against a capture of the original** with a wheel on a prop. No such
  capture exists. The search, the wake and (S169) the owner branch are proved one by one.
- The fallback merge (worlds answering only `World::solid`) is the port's own. Real tracks with props
  hand them over through `World::wooded` and use the shared record.
- When the body rays struck a prop this tick, the wheel reads the bump's pushed copy. If the wheel also
  wakes that prop, `pushed()` writes the copy back after the frame. The copy was made from a struck,
  already-woken prop, so nothing is lost, but the interplay was not compared against the original.
- Only free09's props were checked (beach balls and the rest of its `.toy`). Other tracks' knockable
  props go through the same code.
