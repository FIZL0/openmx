# 1 — bikes wake props (FINAL)

Task S239. 2026-11-16.

## What I did

- Most of the task was done already. I checked each part and only changed what was missing:
  - The sweep probe's category list (exclude mode, two words) and the wreck bike's hull in the props'
    world: S243 (`sweep_categories`, `Scene::probed_among`).
  - free09's trailers are toys (`Prop::moving`). They are already in the props' world.
  - The ridden bike and the AI bikes are heard through `PropSet::hear_ridden`, tested by
    `flight_search::bikes_probed`, and filed with `file_ridden(true)`.
- The gap: a prop's rays striking a bike's frame never fired the probe's two listeners. So an
  asleep prop under a bike never woke. `prop_world.rs`: a bike hit now wakes an asleep body (the
  bike's category has the wake mask's bit) and activates the collection, as any other hit does.
- The second gap, found while proving it: the port tested every bike on every search. The original
  only tests a bike its kept list holds within the probe's reach, at the distance measured at the
  list's last gather (wake, post, asleep check). While a prop sleeps, its list is not gathered again
  until the asleep check runs, so a bike moved beside it is not struck until then.
  - `Collision::gathered_bikes` keeps each bike's transform as of the last gather.
  - `flight_search::bike_listed` decides if that gather left the bike within the probe's reach.
  - `broad_phase::reach_distance` is `collidable_distance` for a bare centre and reach.
  - The field is in the snapshot.

## What the original does

- The bikes' frame collidables (category 0x9a048, flags 0x14) are always gathered into a prop's
  nearby list (record +0x10c), each with its distance from the probe's centre, less its reach. The
  list is in gather order: the static tree (sculpture, plant groups, trailer toys 0x48, …), then the
  moving list (AI bikes, the ridden bike, gates).
- The search skips any entry whose kept distance is beyond the probe's radius
  (`world_search::search_nearby`). A bike that moves near after the gather keeps its old distance.
- An AI bike's collidable keeps its own copy of its transform at +0x120 (origin at +0x150), its
  centre at +0x40 and its frame record at +0x80. The record's node is at +0xf8, its chassis at
  +0xf0, and the collidable back at +0x1f8.

## How it was proved

- `crates/game-sim/tests/prop_bike_wake.rs`, two tests on first_recording's RAM image. The egg on
  its sculpture is put to sleep, and the first AI bike in its list is moved beside it (a spot on a
  ring where the egg's rays meet the bike's frame). Each test rides 120 frames. Every frame it
  compares the egg's list (entry by entry), body, node, contact flags, category list, countdown,
  fresh, active and listeners with the original's. All bit for bit.
  - Bike placed before the gather: the egg wakes on frame 1.
  - Bike moved at frame 31: frame 31's search does not strike it (stale distance). The asleep check
    gathers it near, and frame 32 wakes the egg.
- Mutation: with the list gate removed, the second test fails at frame 31 (the port strikes 88 rays,
  the original none).
- The test loads free09's plants. The woken egg flies into a tree's placed group, which the
  original's rays strike.
- prop_wake, prop_wreck, prop_knock and knock_launch still pass.

## Not verified / not 1:1

- No trailer case was proved. The trailers were already in the props' world, and this task did not
  change them. No test puts an asleep prop beside a trailer.
- The ridden bike was not proved, only an AI bike. They share one code path and one category.
- In the port the bikes are tested after the scene's hits. In the original they come in list order
  (after the static tree and the toys, before the gates). The test's lists matched, but a prop that
  strikes both a bike and a gate in one search might list them in a different order. That was not
  tested.
- `bike_listed` measures the gathered transform from the probe's node at search time, not at gather
  time. While asleep the two are the same. Awake, the post gathers every frame, so they differ by at
  most one step. The original's resort (between full gathers) also measures afresh, so this matches
  in the cases tested, but a fast prop next to a bike was not tested.
- A full list (capacity 128) that would drop a bike is not modelled.

## Next

Nothing. The task is done.
