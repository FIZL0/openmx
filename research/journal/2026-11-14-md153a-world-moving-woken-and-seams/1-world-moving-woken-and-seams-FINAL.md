# 1 — world moving woken and seams (FINAL)

Task MD153a. 2026-11-14.

## What I did

- **Woken cars drawn from the sim** (`crates/openmx/src/world_dress.rs`, `crates/openmx/src/main.rs`).
  - `Moving::car_firsts`: for each tile, where its cars start in the world's joined traffic, and how many.
  - `Dressed::cars_near` takes a `woken` closure. When it gives a pose, the car is drawn there and not
    on its clip.
  - `world_dress::woken_pose`: while the sim's `PhysicsCar` for that car is `physical`, the pose is
    blended from the tick before (`cars_before`) to the body's pose, as track cars are.
  - A tile whose car count differs from the sim's falls back to the clip.
  - Headless ride ends now say `physics cars: …` and `woken car: <model> body x,z drawn x,z`.
- **Seam copies of the moving list** (`crates/game-sim/src/tiles.rs`).
  - `SEAM_COPIES` = 3 places per moving shape, after the originals in every near set and the plane's set.
  - On a wrapped world a shape within reach of a seam gets a copy a width over in those places (from
    `Tiled::copies`, as still shapes do). Unused places are empty meshes with radius 0.
  - The places are always there, so the still shapes keep their numbers whether a copy is in use.
  - A strike heard on a copy is forwarded to the shape it copies (`moving_list`).
  - The sweep already sweeps the moving list and re-sweeps shifted seams, so it needs no copies.
- Follow-up MD166 (low): knocked props' bodies across a seam.

## What the original does

The original has no worlds and no wrap. Seam copies are the port's own. Woken cars are drawn at their
body's pose on a track already (S186); this does the same on a world.

## How it was proved

- `crates/game-sim/tests/wrap_moving.rs`, 4 tests on two generated tiles wrapped on x:
  - a wall 4 ft from the far seam has a copy a width back in the near set and the plane's set. Its frame,
    inverse, sphere and motion match `Prop::on` at the shifted pose, within 1e-3;
  - a wall in the middle has only empty places, and a still shape keeps its number when a copy comes into use;
  - a strike heard on a copy reaches the wall;
  - a world that doesn't wrap has no copies.
- `crates/openmx/tests/world_moving.rs` `a_woken_truck_is_drawn_where_its_body_is`: headless
  world_mxu_free, full gas east from (3950, 2960) head on into free06's truck Car22. It wakes, and is
  drawn within 3 ft of its body.
  - Mutant (the woken closure gives `None`): fails with "mxu_free06::ctruck body 4091.8,2941.3 drawn
    3925.0,3038.6: drawn 193.1 ft from its body".
- The other two world_moving rides (train, beach balls) still pass. `./kit audit` ok.

## Not verified / not 1:1

- Knocked props (PropSet bodies) against the bike across a seam: `prop_contact` has no wrap. Filed as MD166.
- The car contact's extra body-ray pass reads only the master moving list, not the seam copies. Copies are
  met in the scenery search, wheels, the props' search and the plane's set.
- Seam copies are checked only in the unit test, not in a ride across a seam on a real world.
- A copy's `from_world` is shifted by subtraction, not rebuilt by `invert_rigid`, so it may round
  differently from `Prop::on` (the test allows 1e-3).
- 3 places per shape is the port's choice. Each place in use clones a mesh per set.
- Woken drawing applies only while the car is `physical`. A tile whose car count differs from the sim's
  draws the clip.
- The wheels of a woken car still turn on the clip's palette.
- No on-screen look; the proof is the headless lines.
- The world test depends on the truck's timing on its path (it meets the bike about 5 s in).
- No two-seat or netplay run. The picture reads seat 0's traffic.

## Next

None for this task. MD166 for knocked props across a seam.
