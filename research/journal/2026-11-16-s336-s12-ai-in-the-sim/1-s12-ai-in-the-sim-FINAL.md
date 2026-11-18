# 1 — s12 ai in the sim (FINAL)

Task S336. 2026-11-16.

## What I did

- Found that the AI lane's scene already walks and sweeps the map's moving meshes the Sim holds.
  - The Sim's traffic attaches its path cars' meshes and its animated toys' meshes at the end of the
    world's props, and marks each `Prop::moving` every tick before the riders think.
  - `flight_search::world_flown` (S141) files every prop with `moving` set in the moving list, not
    the static tree. `WorldScene::walk` queries both structures (kind 3), and the list is handed out
    whole, with no box. So the cars and toy meshes come after the tree's things, in the props' order,
    as `Thing::Solid { collidable, ai_ignore: false }`, and `WorldScene::sweep` sweeps them as props.
  - So no code change was needed in the walk. I wrote the proof, and said so in `WorldScene`'s doc.
- New test `s336_the_feelers_meet_a_path_car` in `crates/game-sim/tests/ai/moving.rs`.
- research/oracle.txt, under `ai_iface_walk_begin`: what free09's moving list holds, in order.
- Nothing taken from ../opentennis: it has no AI scene walk.

## What the original does

- At recording2's image, the first AI rider's walk (`./kit fn 0x00280aa0`) hands out, after the static
  tree's things, in this order:
  - free09's six path cars: class-0 meshes, category 0x48, all collidable. They are the `.toy`'s
    `free_trailercab`, `free_trailerC`, cab, `free_trailerA`, cab, `free_trailerD`, in that order
    (radii 21.72 / 43.29 / 21.72 / 43.08 / 21.72 / 42.18, matching the port's meshes).
  - the rustic windmill toy's `.col` mesh: category 0x20a, collidable. Its matrix is the port's toy
    placement, bit for bit. (The task called the cars "toys" and this "a 0x20a mesh": it is the other
    way round.) The four SETI windmills have no collision.
  - the parked biplane: a 0x52058 mesh (collidable, not `AI_IGNORE`), its 0x10008 rays, a 0x8 sphere.
  - then the riders' bikes, rays, spheres and ragdoll parts (S326), and non-collidable hierarchies.
- None of the cars, the windmill or the plane mesh has the `AI_IGNORE` bit (0x8000), so the avoidance
  sweeps them all.

## How it was proved

Save-state capture against the original in the interpreter: `s336_the_feelers_meet_a_path_car`, on
recording2's image (S12e).

- The port's free09 cars and toys are built as the shell builds them (`toy::Traffic::load(..).sim`,
  `Animated::load(..).sim`, `Traffic::attach`): 7 props. The original's walk hands out 7 matching
  objects. Each prop, stood at its object's matrix (`+0x120`), has the object's centre (`+0x40`) and
  reach (`+0x60`), bit for bit.
- The first trailer cab is moved in RAM along the first AI rider's heading (6 to 110 ft) and across it
  (-20 to 20 ft), at the rider's height: 90 spots. At each spot, one tick runs on each side from the
  same image: the original's `ai_update`, and `Pilot::think_among` on the port's free09 world with the
  7 movers and the 4 bikes. Per spot:
  - the original walked the 7 movers, none `AI_IGNORE`, before the plane and the bikes; the port
    walked the same 7 props, then its 4 bikes;
  - every sweep at a mover was alike, met or not, feeler by feeler: 630 sweeps;
  - the original swept its parked plane and never met it; no other sweep met, on either side;
  - the tick after was alike: the answer, the generator, both flag words, body velocity, speed,
    steer, pitch, throttle, brake, target speed, four levers, three recovery words, yaw rate.
- 78 of the 90 spots met the cab. The whole tick still matched, so the avoidance's reaction to a car
  on its feelers is the original's.
- The S326 and S323 tests still pass.

## Not verified / not 1:1

- **The parked biplane is not in the port's scene.** The original's walk hands it out between the
  windmill and the bikes, and sweeps it (it is not `AI_IGNORE`). The Sim holds no parked plane: a
  plane exists only while someone flies it (`Sim::plane`). An AI rider riding into the parked plane
  would stop on it in the original and not in the port. In the test the plane was ~570 ft away and
  never met, so the ticks still matched. Filed as S339.
- **The cars' and toy's poses came from RAM**, not from a Sim-stepped `Traffic`. That the Sim's
  `Traffic::frame` puts them where the original does is S81/S145/V43's proof, not this one's.
- **A car or toy before its first frame.** Until `Traffic::frame` or `Toys::frame` has marked a prop
  `moving`, `world_flown` files it in the static tree, inside the box. In a Sim that is never true at
  think time (the traffic steps first each tick), so it was not tested.
- **The moved cab's centre** was written as the port's prop puts it, not as the engine would refresh
  it. The formula is proved on the 7 unmoved objects.
- **Other maps' movers** (knockable props, toy hierarchy parts, a world's tiles' cars) were not
  compared, and I did not check that knockable props reach the AI walk at all. Only free09 was captured.
- **The shack's hierarchy** (free09's utility shack) is still missing from the port's world (S192).

## Next

- A parked biplane in the Sim's world, walked and swept by the AI scene : S339.
