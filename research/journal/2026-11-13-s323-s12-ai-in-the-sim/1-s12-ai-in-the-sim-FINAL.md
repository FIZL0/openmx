# 1 — s12 ai in the sim (FINAL)

Task S323. 2026-11-13.

## What I did

- `ai::Pilot`: one AI rider's carried state. That is the racer (brain, interface, senses, seeker,
  habits), its character and wander, the levers it hands off, the recovery words, the race card, a
  `Seat` (the rider's fixed words: speed scale, slow radius and speed, player, air cone, look-ahead,
  bump chance, race phase, path net, live) and the ignore list. `Pilot::spawn` is S12e's
  `ai::spawn` + `read_character` + `Racer::take`. `Pilot::think` is one tick: `Racer::tick`, given an
  `Around` whose vehicle scene is the Sim's own world, then `ride_controls` from the levers and the
  habits' clutch.
- `ai::WorldScene`: the `Scene` the brain walks and sweeps, built from the Sim's `World`.
  - **The walk** is the port's broad phase (`flight_search::world_flown` over the props and plants),
    queried as kind 3 round the bike's centre of gravity with a half box of `WALK_HALF` (50) on each
    axis. Props come out as `Thing::Solid`, plant groups as `Thing::Group` by their cell, and the
    port's own probes are skipped.
  - **A sweep** is `back_search::sweep_object` with the brain's bundle (centre = the rider's position,
    radius = feeler length, the feelers as probes) against the prop, keeping the per-feeler hits.
  - Ground probes go to `World::ground`. A `Lent` RefCell lets the scene and the ground share the one
    `&mut dyn World`.
- `ai::Lane`: the seek world, the AI's `ragdoll::Tables`, the AI's own `Rng` and the pilots by rider
  index. On `Sim` it is `ai: Option<Box<ai::Lane>>`; None steps and hashes exactly as before.
  `Sim::think` (tick.rs) runs in `step_all`'s rider loop, in rider order (which is actor order for a
  roster). It does four things:
  - sets the rider's `sense.ai_route`;
  - builds the brain's view of the bike (`mount_of`) and the others (`other_of`);
  - lets the assist pull the chassis' yaw rate and velocity;
  - hands back controls, which replace the pad's for that rider.
- `rider.rs`: a trick request goes through `ai::gesture_gate(sense.ai_route, …)`. A human rider's
  gate is the plain `Tricks::request`, as before. The brain's `plan_trick` runs inside `Racer::tick`
  once a frame when `Pilot::think` is handed the rider's tricks, and `Sim::think` hands them.
- `WorldScene::swept` / `Thought::swept` carry each sweep's per-feeler hits.
- tests/ai/sim.rs: the S12e readers are `pub(crate)` so that lane.rs can seed from them. Only their
  visibility changed.

## What the original does

The AI interface's walk-begin thunk (`./kit fn 0x00280aa0`) opens a world-tree query of kind 3:
- the query is held in the rider;
- it walks the rider's world's tree;
- the centre is the body's centre of gravity (body +0x60);
- the half box is a constant quad (50, 50, 50, 1);
- it ends with the "first only" flag 1.

Walk-next (`./kit fn 0x001bb490`) hands out objects one by one, and `0x001bb890` closes the query.
The brain's sweep bundle is made on first use. Its centre is the rider's position and its radius is
the feeler length (100 on free09's riders). The per-feeler hits are kept at the bundle's record. The
details are in research/oracle.txt under ai_iface_walk_begin.

## How it was proved

A save-state capture, stepped. `crates/game-sim/tests/ai/lane.rs`,
`s323_the_lane_thinks_on_the_sims_own_world`:
- recording2's image (S12e), with free09 loaded off the disc into a Sim world (`eco::Wooded` over
  `world::Place{Ground, Props}` and the plants on the image's curve table);
- three riders × 60 ticks = 180 rider ticks, interleaved in actor order, with one shared generator;
- each tick, the original's `ai_update` runs unreplaced in the interpreter. Its ground query, walk,
  class bit and sweeps are only watched.

Against it, `Pilot::think` runs on the port's world. These were equal, bit for bit, on every rider
tick:
- **Ground:** 2340 queries, every point asked and the port's ground's answer (height, and normal where
  the original asks for one).
- **The walk:** the things the port's world holds, in the original's order, with nothing extra. Meshes
  were matched by centre and radius and plant groups by cell. The original handed out 28080 things,
  but near these riders the static tree holds none of the port's props or plants. All 28080 came
  from the moving structure (below), and the port's walk was empty every tick. So this part of the
  stepped test only shows that both sides walk no fixed thing there.
- **Sweeps:** the original swept 16020 times, all at moving objects, and none met. The test asserts
  that none met, because the port's world lacks those objects. No port prop was swept in the
  stepped test.
- **The tick:** the answer, the rand state, both flag words, position, ground normal, ground, speed,
  steer, pitch, throttle, brake, heading and target speed.
- **The handoff:** the five levers and the clutch, the three recovery words, the body's yaw rate and
  velocity after the assist, and the controls `ride_controls` hands the bike.

The plants' cell equals the world manager's group cell.

Because the stepped riders walk no fixed thing, a second test,
`s323_the_lanes_walk_is_the_originals_on_the_fixed_world`, proves the static walk on its own. The
first rider's body centre of gravity is written onto each of free09's 275 props in turn. Then the
original's walk is called through the interface's begin/next/end thunks (`ai_iface_walk_end` was
named for this, in oracle.txt). Its fixed things (the port's props by place and reach, plant groups
by cell) equal `WorldScene::walk` at the same centre, in order, in all 275 walks: 3286 props and 614
plant groups.

**What the original walks:** kind 3 is the static tree inside the box, then the moving structure.
The moving structure hands out every moving object wherever it is: the riders' bikes and ragdoll
parts (a cluster at each rider), 13 objects with w = 0 centres and radius ≈ 43.3 spread over the
map, class 1 and class 6 objects, and zero-centred ones. That is about 156 things per rider tick.

## Not verified / not 1:1

- **The Sim-stepped bodies.** `Sim::step_all` feeds the lane's controls into the bike, but nothing
  compares the bodies that come out with the original's. The test freezes the bikes, as S12e's join
  does. The AI bikes' physics (their .phy and the A-physics bodies) and the capture are S12e's own
  proof, which waits on C33's free09 4-racer capture.
- **`mount_of` and `other_of`.** The frame from `ride.transform()`, the on-ground bit and
  `chassis.speed` as the others' speed are unproved. In the test the brain's view of the bike was read
  from RAM (S12e's `s12e_bike`), not built by `mount_of` from a port rider. `other_of`'s race progress
  is 0, which is only right off a race.
- **The moving structure is not in the lane's scene.** The original's kind-3 walk also hands out
  every moving object: riders' bikes and ragdoll parts, the radius ≈ 43.3 objects with w = 0
  centres, class 1 and class 6 objects, and zero-centred ones. It sweeps the collidable class-2 ones.
  The port's `WorldScene` walks only the fixed world. The stepped test checks only that none of
  those sweeps met in these 180 ticks. Riders that come within a feeler of another rider's bike would
  part. free09's utility shack (a Hierarchy the port's props skip, S192) is missing too, so it is not
  walked or swept either. S324.
- **No sweep against a port prop has been compared.** `back_search::sweep_object` is proved on its own
  (S192, tests/back_search.rs). The lane's bundle (centre = position, radius = feeler length, probes
  from the fan) follows the original as read, but no stepped rider swept a prop, so it was never
  compared with a met sweep.
- **The ignore list and the AI_IGNORE bit.** Both are empty or unset on free09, so the walk's
  `ai_ignore: false` and the ignore filter are not exercised.
- **The walk's cost.** The walk rebuilds the broad phase (`world_flown`) on every call. That is
  correct, but it is slow for many riders, and the cost was not measured.
- **`Lane` is not in the snapshot or the state hash** (tests/hash.rs pins the Sim at 12264 with it). A resync keeps each machine's own lane, so an
  online ride with AI riders could drift. Nothing rides online with AI yet.
- **No ride builds a `Lane` yet.** Spawning the roster at a map load, with S313's seek loader, is
  S12e's and S316's work.
- `ai::Rider::look_ahead` / `bump_chance` come from the seat (read once at spawn). They were not
  checked against later changes of the rider's words.

## Next

S12e, once C33's capture exists: build a `Lane` from the roster at the map load (S316), step the
bodies through the Sim, and compare the AI bikes' bodies and the rand state with the capture. Put
the moving objects (riders' bikes and parts) in the lane's scene walk first (S324).
