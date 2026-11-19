# 1 — parked truck in the ai scene (FINAL)

Task S461. 2026-11-18.

## What I did

- Found that the thing S336 called free09's "parked biplane" is the parked **monster truck**.
  - Its `0x52058` mesh is named `monstertruckf.col` in the object (`+0x88`), its rays `monstertruckm.col`.
  - `0x52058` is the car's `car::COLLIDABLE`.
  - The car stands at free09's `machine.txt` `[Machine1]` (Type Monster, 4594, 135, 3838, nose -z).
- So I did the task for the truck. The Sim now holds parked machines' hulls, and the AI scene walks
  and sweeps them.
  - `ai::parked_hull(mesh, &body)`: a machine's `f.col` as a `world::Prop`, stood at its body's pose
    (`wreck_bike::node_of`, as `car::Machine::hull` places a driven car's hull).
  - `Sim::parked: Arc<[world::Prop]>`: fixed for the ride, empty by default. Not walked, hashed or
    sent (like `Sim::home`). The size pin in tests/hash.rs went 12376 → 12392.
  - `ai::WorldScene::parked`, with ids from `ai::PARKED` (0x4000_0000). The walk hands them out after
    the moving props and before the bikes, collidable and not `AI_IGNORE`. The sweep is `sweep_object`.
  - `Pilot::think_amid` is `think_among` with the parked list. `think_among` calls it with none, so its
    callers are unchanged. `Sim::think` (tick.rs) passes `Sim::parked`.
- Small changes outside the task's files, needed for the build: tick.rs (the call), snapshot.rs (the new
  field), tests/hash.rs (the size pin), tests/ai/lane.rs (a `WorldScene` literal).
- The S336 test's comments and messages now say "truck", not "plane". Its constant is `PARKED_TRUCK`.
- research/oracle.txt, under `ai_iface_walk_begin`: the truck's car object and its words.
- Nothing taken from ../opentennis: it has no AI scene walk and no parked machines.

## What the original does

- At recording2's image, the first AI rider's walk (`./kit fn 0x00280aa0`) hands out the truck's
  hull (class 0, cat 0x52058, collidable), then its rays (class 2, 0x10008), then a sphere (class 3, 0x8).
  This comes after free09's 6 path cars and windmill, and before the riders.
- The car object is the hull's owner: the hull's `+0x24` is the car's `+0x58` (car 0x00e1e210).
  - It holds its body at `+0x160`, its rays, hull and sphere at `+0x174`, `+0x178` and `+0x17c`.
  - Its spawn is at `+0x270`: (4594, 141.38817, 3838), the file's x and z. Its nose is at `+0x280`: (0, 0, -1).
- The body is asleep (word 0 is 0). It rests about 7 ft under the spawn, pitched 0.0067.
  So the original stepped the parked car from its spawn until it slept.
- The hull is not `AI_IGNORE`, so the avoidance sweeps it, and a feeler can stop on it.

## How it was proved

Save-state capture against the original in the interpreter: `s461_the_feelers_meet_the_parked_truck`
in `crates/game-sim/tests/ai/moving.rs`, on recording2's image (S12e).

- The walk hands out 7 movers, then 1 parked hull, then 4 bikes. The hull is the car's (`+0x178`).
- The car's spawn is `[Machine1]`'s x, z and nose, read off the disc's `free09\machine.txt`.
- `ai::parked_hull` of the body read from RAM gives the hull object's matrix (`+0x120`), centre
  (`+0x40`) and reach (`+0x60`), bit for bit.
- The hull was moved in RAM across the first AI rider's way: 6 to 110 ft along its heading, -20 to
  20 ft across, 90 spots. At each spot, one tick ran on each side: the original's `ai_update`, and
  `Pilot::think_amid` with the 7 movers, the truck and the 4 bikes.
  - Walk order alike on both sides: movers, truck, bikes. The truck is not `AI_IGNORE`.
  - 90 sweeps at the truck, plus every sweep at the movers, alike, met or not, feeler by feeler.
  - No other sweep met, on either side.
  - The tick after was alike: answer, generator, both flag words, body velocity, speed, steer, pitch,
    throttle, brake, target speed, four levers, three recovery words, yaw rate.
- 78 of the 90 spots met the truck, so the avoidance's reaction to it is the original's.
- The whole ai suite and tests/hash.rs pass.

## Not verified / not 1:1

- **Nobody fills `Sim::parked` in play yet.** The shell does not hand the Sim its parked machines,
  so AI riders in the app still ride through the truck. Filed as P378.
- **The rest pose is not the Sim's own.** The original spawns the truck 7 ft up and steps it until
  its body sleeps. The port has no settle or sleep for a parked car. The test reads the resting body
  from RAM, as S336 read the cars' poses. Filed as S484.
- **Bikes do not hit the parked truck.** It is only in the AI scene, not in the bikes', the plane's
  or the props' worlds. I have no capture of a bike riding into it. Filed as S484.
- **Which machines are parked** is unproved. recording2's walk holds only `[Machine1]`'s truck, not
  the `Biplane1`/`Buggy1`/… bays the shell's lot parks. I did not find out what picks them
  (maybe `NumberOfMachines`, maybe the mode). Filed as P378.
- **The truck's rays and sphere** are not walked by the port. The original hands them out and the
  avoidance may sweep them. The per-object sweep answers only meshes and hierarchies, as S326 found for a
  bike's rays, so they change nothing the brain reads. Not checked one by one here.
- **A driven truck.** While someone drives the parked machine it should leave the parked list. Nothing
  does that yet (P378).
- **The moved hull's centre** was written into RAM as the port's prop puts it, not as the engine
  would refresh it. The formula is proved where the image has the truck.
- **`Sim::think` passing `Sim::parked`** is one line, and no test runs a whole Sim with a parked
  hull. The test drives `Pilot::think_amid` directly.
- **Other maps' parked machines** were not compared. Only free09's truck was captured.

## Next

- S484: the parked car's settle and sleep, and bikes against it.
- P378: the shell fills `Sim::parked`.
