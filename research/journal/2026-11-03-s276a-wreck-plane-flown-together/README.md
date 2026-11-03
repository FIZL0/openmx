# S276a — the wreck plane among planes flown together

2026-11-03. Task: `./kit plan S276a`. Follows S276 (`research/journal/2026-11-03-s276-wreck-plane-in-the-lanes`).

S276 let a downed rider's wreck bike meet a lone flown plane. Planes flown together (two or more in
one `flight::frames_together`: `vehicle::fly_together`, the shell's `fly_held`, online's `fly_held`)
did not meet it. Now they do.

## What changed

- **`wreck_plane::search_downed_by`.** It takes several planes and their `Met`s.
  - Each wreck bike is searched once, with every hull in its world, in the planes' order. A plane with
    no hull is left out.
  - The `search_flown` record capacity is the largest of the hulls'.
  - Plane `k`'s mirrors go on `mets[k].mirrors` as `WRECK_FROM + j`, `j` being the body's place in
    `mets[k].wrecks`. Each mirror's point is moved to the copy nearest that plane.
  - The wreck body goes into every searched plane's `Met::wrecks`, under one `object` (its place among
    the searched).
  - `Downed` now keeps, for each searched rider, its `(plane, j)` places and the hulled planes' order.
- **`wreck_plane::step_downed_by`.** It runs after the joint frame.
  - Each wreck bike takes back one body: the first of its places. The joint frame left them all the same.
  - It steps (`step_flown`) with every hulled plane's `flown` body, in the search's order. That order is
    what the entries' `PLANES + j` names.
  - Each plane's body is written back (`Flight::pushed`).
  - `search_downed` / `step_downed` are now these with one plane, so the lone path is unchanged
    (pinned below).
- **`flight::frames_together_props`.** Each plane's `Met::wrecks` goes into the fleet's one list of
  pushed bodies, after the props.
  - A wreck body two planes hold is one body, found by `object`, so both resolves push it substep by
    substep, as a prop struck by two planes is pushed.
  - Each plane's `WRECK_FROM + j` mirrors are renamed to `vehicles + (n-1) + place`, its resolve's side.
  - After the step, each plane's met gets the bodies back, and they are cut off before
    `PropSet::pushed`.
- **Fixed on the way: `Flight::knocked` took the wreck mirrors for props.** It renames every
  `other >= KNOCK_FROM`, and `WRECK_FROM` (1<<21) is past `KNOCK_FROM` (1<<20), so a wreck mirror was
  renamed to `None` whenever a plane flew among the knockable props. That covers the lone `frame_props`
  too, since S276. Both of its scans now stop at `WRECK_FROM`.
- **The shell's `fly_held`.** It now takes the walk's `on`.
  - The riders are the seats `on` holds that do not fly; the flying seats walk first, so their own pass
    comes later.
  - It searches before `fly_together` and steps after (`own_step = true`). Then `settle_plane` poses
    each plane as before.
- **Online's `fly_held`.** It now takes `touching`. The riders are the touching seats not held: as
  `others_of` for a lone plane, but leaving out every flyer. The rest is the shell's.

## Proof

`tests/c16_fly.rs`, `planes_flown_together_meet_p2s_wreck_bike`, starts from s272-137910.ram with P2
down (wreck 7), Cross held, 10 ticks at 1/60.
- It flies plane 0 alone: `search_downed`, `frame_met`, `step_downed`.
- It flies plane 0 with a second plane 400 ft above, in one `frames_together`: `search_downed_by`,
  `step_downed_by`. Plane 0 is first in the fleet on one run and second on the other.
- On both runs, plane 0's velocity and position and the wreck bike's velocity and position are bit for
  bit the lone run's, every tick.
- The lone run's plane is `the_sim_lane_meets_p2s_wreck_bike`'s Sim-lane plane, bit for bit (both
  printed). Its first tick, -24.854322, 8.510814, 2.1630852, includes the wreck mirror.

The run used the image copied from slot s2's `context/states/s272`. Elsewhere the test skips.

Also run: `c16_fly` (all), `plane_meets_plane`, `prop_plane`, `flight_bikes`, `openmx --test
plane_seat_props` and `openmx --lib together`. All pass.

## Not verified / not 1:1

- **The original has no joint-frame capture with a wreck bike under two planes.** The test proves only
  that the joint path reduces to the lone one when the second plane is out of reach. With two planes
  both touching one wreck bike, the order of their resolves on its one body follows the fleet order of
  `frames_together`'s props. It is not checked against the original.
- **The wreck bike's own search takes every hull with one record capacity, the largest.** The
  original's per-plane records may differ, but both planes are the same biplane file today.
- **The shell's and online's `fly_held` are checked only by the build and the tests above.** No test
  drives split screen or a netplay ride with two planes over a wreck.
- **Online's `fly_held` called at the end of the walk** (every seat aloft) uses `own_step = true`. Then
  no downed rider steps later this tick, so the `flown` mark would carry into the next tick. That can
  only happen if a seat flying a plane also has a crash, which a plane seat does not keep. It is not
  guarded.
