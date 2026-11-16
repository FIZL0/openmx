# 1 — s12 ai in the sim (FINAL)

Task S326. 2026-11-16.

## What I did

- `ai::Ridden`: one rider's bike as the AI's scene walk sees it. It holds the bike's `BikeCol` (its
  `bikef.col` frame mesh) and the chassis transform.
  - `Ridden::of(rider)` gives a bike only when the rider carries collision files, is up on it (no
    wreck, no crash) and is not a ghost. This is the same rule `rider_contact` uses.
  - `Ridden::sphere()` is the frame object's centre and reach (`+0x40`, `+0x60`): the root box
    through `broad_build::mesh_box`, as a prop's, and `BikeCol::frame_radius`.
- `ai::WorldScene::ridden`: after the static tree's things, the walk hands out every bike, wherever
  it is. Each is `Thing::Solid { id: MOVING + k, collidable: true, ai_ignore: true }`.
  - `WorldScene::sweep` answers a `MOVING` id with `back_search::sweep_placed` against the bike's
    frame. The avoidance never asks it, because the bikes are `AI_IGNORE`.
- `back_search::sweep_placed`: `sweep_object`'s body, for any placed mesh. `sweep_object` now calls
  it with the prop's mesh, matrix and sphere. Nothing else changed in it.
- `Pilot::think_among(.., ridden, ..)`: `Pilot::think` with the bikes. `think` keeps its signature
  and passes none.
- `Sim::think` (tick.rs) builds the bikes of all the Sim's riders, in player order, the thinking
  rider's own bike included, and calls `think_among`.
- research/oracle.txt, under `ai_iface_walk_begin`: the brain bundle's probe arrays, what the moving
  walk holds, and the `AI_IGNORE` bit.

## What the original does

- The walk (`./kit fn 0x00280aa0`, S323) is the static tree inside the box, then the whole moving
  structure. At recording2's image, the first AI rider's walk hands out, for each of the 4 riders:
  - its bike frame (class 0, a mesh, category 0x9a048, radius 4.3035);
  - its wheel rays (class 2, 0x18008);
  - a sphere (class 3, 0x8).
- The same walk also hands out:
  - ragdoll parts (classes 3 and 5, 0x8);
  - free09's toys (0x48 meshes, radius 21.7 and 43.3);
  - a 0x20a mesh;
  - the parked plane (0x52058 mesh, 0x10008 rays, 0x8 sphere);
  - non-collidable hierarchies (0x200020), and kind-1 and kind-6 objects.
- The avoidance (`ai_update_vehicle`, S317) asks `class_bit("AI_IGNORE")`. It is **0x8000** on every
  tick of the test. It skips any type-2 object whose `+0x84` has that bit.
  - The bike builder ORs `[0x4e2128] | [0x4e2130]` = 0x8000 | 0x10000 into the frame, both ray
    probes and the wreck part (S93's notes, oracle.txt). So the frame (0x9a048) and the rays
    (0x18008) are all `AI_IGNORE`.
  - Nothing clears that bit: the ghost switch edits the `+0xe8` lists, not `+0x84`.
  - So the original never sweeps a bike, its own or another's. AI riders keep apart only through the
    race list's `others` pass (`Other`).
- The per-object sweep (`./kit fn 0x0011c210`) meets only class 0 (mesh) and class 1 (hierarchy). It
  returns 0 for classes 2 and 3, and for any other class after the mesh branch. So the rider spheres
  and ragdoll parts, which are swept (category 0x8 is not `AI_IGNORE`), can never meet.
- The brain's bundle (`+0x2a0`): count `+0x120`, lengths `+0x134`, origins `+0x138`, directions
  `+0x13c` (w 1.0), centre `+0x150`, radius `+0x160`. The record is `+0x6c`, with per-feeler hits
  at its `+0xa4`.

## How it was proved

Save-state captures against the original in the interpreter. Both tests are in
`crates/game-sim/tests/ai/moving.rs`, on recording2's image.

- `s326_a_bikes_sphere_is_the_objects`: the first AI rider's walk hands out 4 bike frames. All are
  collidable, and `Ridden::sphere` of each frame's matrix (`+0x120`) equals its `+0x40` and `+0x60`,
  bit for bit.
- `s326_the_feelers_meet_another_riders_bike`: the second AI rider's bike frame is moved in RAM
  (matrix translation, and its centre as `Ridden::sphere` makes it). It goes 6 to 110 ft along the
  first rider's heading and -20 to 20 ft across it: 90 spots. At each spot, one tick runs on each
  side from the same image: the original's `ai_update`, and `Pilot::think_among` with the 4 bikes
  read from RAM, on the port's free09 world. Per spot:
  - the original walked the 4 bikes, all collidable with the `AI_IGNORE` bit (= `RIDE_START[0]`), and
    the port walked them as `Solid { MOVING + k, collidable, ai_ignore: true }`;
  - neither side swept a bike. The original's 8010 other sweeps (89 a tick) met nothing, and neither
    did the port's;
  - the tick after was alike: the answer, the generator, both flag words, speed, steer, pitch,
    throttle, brake, target speed, four levers, the three recovery words, and the yaw rate and
    velocity after the assist.
- The sweep the avoidance would make if bikes were not `AI_IGNORE`: the tick's own bundle (read back
  from RAM), swept at each bike through the original's `bundle_sweep_object` and the port's
  `WorldScene::sweep`. All 360 sweeps were alike, met or not, feeler by feeler. 153 met, and 63 of
  the 90 spots met the moved bike.
- The S323 tests still pass. The whole `--test ai` suite passes (57 tests).

## Not verified / not 1:1

- **The task's premise is not the original's.** The task said the walk sweeps the collidable moving
  objects, so an AI's feelers would meet another rider's bike. The original's avoidance never sweeps
  a bike, because all of them are `AI_IGNORE`. The port does the same. The meeting is proved only at
  the sweep (the bundle at each bike, above), not as something the avoidance does. S323's note
  "riders within a feeler of another rider's bike would part" was wrong.
- **The bikes' order in the walk.** The port hands them out in the Sim's player order. The original
  uses its moving list's order, which at recording2's image was not player order (rider 2's bike,
  the fourth rider's, rider 0's, rider 1's). In the test the port's bikes were listed in the
  original's order, so order was never compared. It changes nothing the brain reads: the bikes are
  skipped, and a block only zeroes a feeler's speed and ORs a reason bit, which do not depend on order.
- **Not walked: the rest of each rider, and the map's other moving things.** Wheel rays, the rider
  sphere and ragdoll parts are left out. They are `AI_IGNORE`, or swept without ever meeting
  (classes 2, 3, 5), so they cannot change the tick. The toys (0x48 meshes), the 0x20a mesh, the
  parked plane and the shack's hierarchy are left out too. Those **can** meet a feeler (meshes, not
  `AI_IGNORE`), but the Sim has no free-ride toy or parked-plane objects for the scene. Filed as a
  follow-up as S335 (see Next).
- **`Ridden::of`'s transform.** It is `ride.transform()`, the transform `rider_contact` and the
  plane's world put the frame on (S78, S95). In this test the matrices came from RAM, not from a
  Sim-stepped rider, so `Sim::think`'s bikes were never compared with the original's.
- **The moved bike's centre** was written as `Ridden::sphere` makes it, not as the engine would
  refresh it. The formula is proved on the 4 unmoved bikes only.
- **A wrapping world (MD44).** A bike is not put at its nearest copy as the plane's world does. On a
  wrapping world the bikes are never swept anyway, so only the walk list differs.
- **`Lane` still is not hashed or snapshotted** (S323). `Ridden` is built fresh each tick and holds no
  state.

## Next

- The free ride's other moving meshes (toys, the parked plane, the shack's hierarchy) in the AI
  scene, once the Sim has them: S335.
