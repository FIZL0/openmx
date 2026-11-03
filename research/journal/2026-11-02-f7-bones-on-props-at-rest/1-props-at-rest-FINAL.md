# 1 — the bones against the props at rest (FINAL)

Task F7. 2026-11-02.

## What the original does (read in the interpreter on recording2's free09)

- recording2's static tree holds all 62 of free09's knockable props' mesh collidables (category 0x18,
  vtable mesh, `+0x31` bit 0 set, `+0x7c` 0, `+0x80` the collision object). Their `+0x116` is 0x04 with
  0x40/0x80 bits on some (0xc4) and 0x10 on one; bit 0x08 is clear on every one, so
  collision_search_gather lists them wherever the query's cells reach them, their distance not asked
  (as the wreck bike's 0x14). The test checks 0x04 set and 0x08 clear only.
- Their place in the walk: filed after the scene meshes and the plants. Filing them first breaks the
  list's order (a control, below).
- The sphere pair on a sleeping prop is the same mesh walk as on an awake one (S240).
- The struck listener: collide_against_search on a list holding a sleeping prop alone, the sphere
  touching it, takes the record and leaves the prop's body enabled (it was 0) with its position,
  velocity, orientation and spin unchanged, the collidable's flags then 0x14 (0x54/0x94/0xd4 where the
  high bits were set). So the strike wakes, and pushes nothing: PropSet::struck.
- The push-out against an other with a `+0x80` collision object is S105/S240's moving path
  (ragdoll_contact::moving, Against::Other with Side::Body: transform_rotate / transform_unpoint of the
  arm through the body's orientation). For a sleeping body it is still (its velocity and spin are what
  it fell asleep with). Only the bone is pushed.

## What I did

- `ragdoll_scene.rs`:
  - `Statics::knock_from` (where the props go in `Statics::props`: past the world's props and its
    plants), `Statics::knock_keys` (each sleeping prop's key from its mesh's root box where its node
    stands it), `knock_index` (a body's collection and object from its index).
  - `BikeSearch::gather_knocking` (`gather_wooded` calls it with none): an entry at or past
    `knock_from` is listed with no distance test.
  - `KnockedProp::resting`: the listed sleeping props still asleep, within the frame's reach, each with
    its `place` in the list. `KnockedProp::place` is `None` for S240's awake ones.
  - `WreckBikeScene::touching` walks the static props and the sleeping props by place, the bike where
    the walk met it, then the awake props and the cars. A sleeping prop that takes the record is marked
    struck; its velocity is S240's (Against::Other on the copied body).
- `ragdoll_ride.rs` `Down::step_posed_among`: a regather's keys add the knockable set's sleeping props;
  the scene's `knocked` is the resting ones, then the awake ones; the struck ones are woken after the
  step (as S240).
- The snapshot and hash types are unchanged: `Statics::props` already carries every listed index.

## How it was proved

`crates/game-sim/tests/ragdoll_rest.rs` (recording2.p2m2_SaveState + the disc + MX.PSR):
- `the_bones_search_lists_the_props_at_rest_as_the_originals`: 3,000 probes (three in four within
  40 ft of a sleeping prop, the rest round a tree), each gathered fresh and then kept: 6,000 lists with
  the same scene meshes, plants and sleeping props in order with the same distance bits, centre and
  bound. 10,162 sleeping props listed, 8,080 of them past the bound; 1,122 lists mix them with scenery
  or plants; none full (at most 29 entries).
  - Control, gather never listing them: fails at case 0 (the original lists prop 1039 at 19.6 ft).
  - Control, their keys filed before the props and plants: fails at case 5 (order).
- `the_sphere_test_on_a_prop_at_rest_answers_as_the_originals`: 20,000 spheres near a sleeping prop's
  vertices through collide_against_search with lists [prop], [prop, prop], [nearest scene mesh, prop],
  [prop, nearest sleeping prop], against the port's walk: count, with, held, point and normal bit for
  bit (12,709 hits; the second took 3,271, the first kept 6,237; 8,211 entries gated by distance).
- `a_bones_strike_wakes_a_prop_at_rest_and_pushes_nothing`: 13 props, a fresh state each: woken, body
  words unchanged; the port's PropSet::struck enables the same body.
- `a_ragdoll_thrown_at_a_prop_at_rest_stops_on_it_and_wakes_it`: free09's largest lone sleeping prop
  (collection 59, model 162, reach 6.67), the ragdoll thrown at 25 ft/s from 3 ft off its reach: the
  main bone stops at 1.00 ft short of its centre for ticks 13–17 and slides down it, and the prop wakes;
  without the props it flies 10 ft past and the prop sleeps. The port's own ride, not a capture.
- The existing ragdoll tests (ragdoll, ragdoll_ball, ragdoll_trees, ragdoll_props, ragdoll_cars,
  plane_world_cost) and replay: see the commit message.

## Not verified / not 1:1

- No capture of the original's ragdoll striking a sleeping prop: the ride test is the port's.
- The struck listener runs after the bones' step, not at the hit. For a sleeping prop that matters only
  if a later sphere of the same step read it differently awake: the pair and the push-out do not read
  `enabled`, so no difference shows; the wake's world-node order among other props woken the same frame
  is as S240's.
- A prop that falls asleep again is filed in the original's tree at that moment (after everything
  filed before); the port files every sleeping prop after the plants in collection order. Only the
  order among sleeping props and ties at one plane distance would part, and only for props that woke.
- A listed sleeping prop that wakes while the list is kept: the port drops its static entry (the awake
  set lists it); the original's list keeps the pointer too, and would offer it twice, which changes a
  record only through the listener (a second test is never strictly nearer).
- recording2's one 0x10-flagged tree entry and the 0x40/0x80 bits were not read further.
- The list's capacity: the port still lacks the trigger boxes, the other bikes and the shack (S206).
