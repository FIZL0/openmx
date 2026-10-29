# 1 — world car poses (FINAL)

Task MD45. 2026-10-29.

## What I did

- Found V96 (landed after MD28 filed this) had already given world cars a ground memo
  (`toy::Path::pose_kept`, `toy::Kept`): warm-up frames moved onto the clock's grid of scene steps,
  the ground under each kept. It did cut the queries (1.5 a car a picture), but nothing tested it,
  and it moved the frames themselves, so the heading and up were filtered from different points
  than `Path::pose`'s. On MD45's test it turned a car's 15 ft corner up to **21.9 ft** off
  `Traffic::placed` in some pictures (place exact).
- Rewrote `pose_kept` so the frames are `Path::pose`'s own (`clock − k·step`), the heading is computed
  from them exactly as `pose` does (path only, no ground: bit-identical), the ground is asked exactly
  at the newest `KEPT_ASKED = 3` frames (the car's place is frame 0's, so it is `pose`'s exactly), and
  only the older frames' ups — the ones the normal filter weighs at `0.8·0.2^k`, k ≥ 3 — are read off
  the grid memo: the two grid steps either side, blended by the clock's fraction of a step (every
  frame has the same fraction), normalised. One new grid query a car a scene step, so 3 + ~0.5 a car
  a picture at 60 fps, against `WARM + 1 = 49` for `pose`.
- Why three asked frames and not one: with only frame 0 asked, faceted ground (one flat normal a
  triangle, as a world's `Tiled` ground answers) put a corner 0.068 ft off when a blended up straddled
  a triangle edge; with three it is 0.0027 ft.
- Doc comments on `world_dress::CARS_NEAR` / `Dressed::grounds` updated. Nothing in the simulation
  is touched; `pose_kept` is draw-only.
- Took nothing from opentennis: it has no path cars.

## What the original does

Not applicable: the original keeps the heading and up as state frame to frame; replaying `WARM`
frames is the port's own (toy.rs module note). This task only makes the port's replay cheaper.

## How it was proved

`crates/openmx/src/world_dress.rs` test
`world_cars_ask_the_ground_a_few_times_a_picture_and_stand_where_they_did` (run with `--bins`): a
truck (TimeScale 0.95) and a three-car lagged train on wobbling loops over faceted hills, tile shift
(1200, −300), 1200 pictures at 60 fps (a lap wrap of the train's clock inside), each picture's
`Dressed::cars_near` against `Traffic::placed` (today's `Path::pose`, all 49 frames on the ground).
Measured: first picture 200 queries (4 × 50), then 3.53 a car a picture; place 0 ft off, a 15 ft
corner at most 0.0027 ft off. Asserts: < 3.7 a car a picture, < 0.01 ft for both.

## Not verified / not 1:1

- **The hud ms on world_mxu was not re-measured.** The proof is the query count (49 → ~3.5 a car a
  picture) on a synthetic world; no `--timings` run on world_mxu under load was made, so the 5–6 ms
  MD28 saw is not shown to drop by a number.
- **The turn is not bit-identical to `Path::pose`**: frames 3..48 back read a blended, normalised up
  rather than the ground's at that point. Within 0.01 ft at a 15 ft corner on the test's ground;
  ground steeper or more finely faceted than it could part further. A blend across a clip's end on a
  path that does not loop (the clock wraps) blends two unrelated ups; gated by the filter's 45° test
  like any up. Draw-only, so no simulation or replay effect.
- **A lap wrap (the clock going back) refills the grid** (~47
  queries a car that picture), as V96's did.

## Next

Nothing.
