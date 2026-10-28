# 1 — bone sweep class filter (FINAL)

Task S166. 2026-10-28.

## What I did

- Traced the bone collision's segment sweep (`./kit fn 0x1f7400`) down to its search
  (`./kit fn 0x11a9d0`) and the probe's class filter (`./kit fn 0x119d70`). **The task's premise does
  not hold.** The original's bones never sweep against the wreck bike or a path car, because their
  probe includes only the scenery's class. A fast bone goes through a wagon's thin wall in the game
  too, so a port that stopped it would not be faithful.
- Ported the sweep anyway, with the filter: `ragdoll_scene::sweep_through(from, to, includes, list)`
  is the search, built from S47's `sweep_probe::clear` / `pair` on each listed `Candidate` mesh whose
  class passes `includes`. `class::{SCENERY, PATH_CAR, WRECK_BIKE, BONE_SWEEP}` carry the classes.
  `WreckBikeScene::sweep` lists the bike (through its collidable's sphere, which the ride now fills
  from `kit.bike`) and the cars. With the game's include word it returns None, as the original does.
  `PathCar` gained the collidable's sphere (`centre`, `radius`).
- Follow-up S205: once S165 puts the static props in the bones' scene, list them to the sweep as
  `class::SCENERY`.

## What the original does

A bone collision with sweep on (+0x24) and a segment probe (+0x8c) builds a segment from
`pos - vel * (+0x28)` to `pos`, hands it to the probe, and runs the probe against its search (+0x84):
it clears the probe, then pairs every listed collidable within half the segment's length whose class
passes the probe's class list. A strike (probe +0x116 & 0x20) marks the contact scene and swept, takes
the record's nearest point and normal, and pushes the bone out.

The bone collision's constructor (`./kit fn 0x1f66c0`) sets the probe's list to include mode with a
single mask, the word at 0x468f60 (0x20). Class 0x20 is the track's static scenery: its 161 mesh
collidables are 0x220. Path cars are 0x48 (all 21 in s89-along). The wreck bike's collidable is 0x9a058
(s62 front 2603). Neither has bit 0x20, so neither is ever paired.

## How it was proved

Function oracles: the original's `bone_collision_sweep` against the port's `Contact::solve` on a
scene of the same list, comparing swept, scene, the bone's position and velocity, and on a hit the
point, normal and dv, bit for bit.

- `tests/ragdoll_cars.rs` `the_bones_sweep_through_the_cars_as_the_originals`: s89-along, 20,000
  segments near car vertices, listing one car, two adjacent cars or one car twice. 10,000 run with the
  game's include list and none is swept. The other 10,000 widen the include word to 0x8 in RAM and in
  the port: 3639 swept, 762 of them with two cars listed, and 7559 lists had a car off at a distance.
  It also asserts every bone probe's list is (include, 1 mask, 0x20) with no listeners, and that
  every car is class 0x48.
- `tests/ragdoll_wrecks.rs` `the_bones_sweep_against_the_wreck_bike_as_the_originals`: s62 front
  2603, 10,000 segments near the wreck bike's mesh over the nine bone collisions. Asserts the bike's
  class is 0x9a058. As the game has it, none is swept. Widened, 1075 of 5000 are swept, all bit for bit.

## Not verified / not 1:1

- **A segment ending exactly on a face** (fraction == 1) strikes without latching a nearest. The
  original then reads a stale record left from an earlier sweep; the port returns None. Neither
  oracle hit it, and it cannot happen in the ride because nothing passes the filter.
- **The candidates' own ignore and class lists** (the filter's other direction) are not modelled. The
  cars (exclude mode, no masks, one non-probe ignore entry) and the bike pass them in the states read,
  so the class word decides it.
- **The segment start's `w`.** The port keeps `pos.w` (xyz only), which the replays in ragdoll_wrecks.rs and
  ragdoll_riding.rs confirm against the original's sweep ends. A four-lane `pos - vel*back` that I tried
  parted them, so my reading of the `vsub`'s dest field was wrong. I did not re-read it.
- **0x119be0** (the probe's broad-phase refresh before the search) is not modelled. The port's search
  takes its list as given.
- **No live capture of a bone sweep.** Both proofs are function oracles on save states, and the
  widened half drives the original outside what it ever does.
- **The sweep never strikes anything in the port's ride yet.** The scenery it would strike is not in
  the bones' scene (S165, then S205).
- **The bike's sphere in the ride** comes from `kit.bike`, as S105's sphere test does. It is unused
  while the filter drops the bike.

## Next

Land. S205 picks up the scenery once S165 has it.
