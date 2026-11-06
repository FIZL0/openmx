# 1 — bone collision (FINAL)

Task S278a. 2026-11-05.

## What I did

- `crates/game-sim/src/handling/bone.rs` (new): MX vs. ATV's rider ragdoll bone collision on RAM, the
  source of every part's touched word (`+0x14`) that `handling::rider::touching` reads. It covers `solve`,
  `sweep`, `moving` (the other side's velocity at a point), `push_out`, `impulse` and its `sin_cos`. The
  scene's calls go behind a `Scene` trait: placing the sphere and its refresh, the probe's segment and
  refresh, the pool and whole-scene searches, and a mover's two virtuals.
- `node::unplace`: a world point into a node's frame, `place`'s inverse (the `./kit fn -p atv 0x001953c0`
  shape: update, then the transposed rows on the point less the position).
- Tests in `tests/mxvatv_bike_ride.rs`: `the_bone_collision_is_the_originals` (the oracle) and
  `the_bone_collisions_touch_as_the_originals` (C15 with the port serving every solve).
- research/oracle.txt `# S278a`: the layouts and addresses.
- Follow-ups: S279 (a `Scene` on atv_world's rider spheres) and S280 (the rest of the rider world step).

Why a new RAM module rather than `ragdoll_contact.rs`: the rider's other words (S278's `rider.rs`) and
every C15 replay port in `handling/` work on the interpreter's RAM through `node::Ram`. The bone
collision has to run inside the original's world step until S280 ports that step. A typed port on
`ragdoll_*`'s structs would need the whole world moved off RAM first. The arithmetic follows MXU's
`0x001f6c40`/`0x001f6860` instruction for instruction, as `ragdoll_contact.rs` does.

## What the original does

`./kit fn -p atv 0x0049bb00` (the solve, the part's vtable `0x003cd080` `+0x18`):

1. It returns unless the part is on (`+4`), has a body (`+0x7c`) and that body is enabled.
2. It clears the other side and swept, then takes the point from the body's position.
3. With a terrain (`+0x78`) and the point strictly inside its box (x, z), it runs the ground query
   (`# MD76`) with the part's surface byte. Point, normal and byte are written back. Below
   `point.y + radius`: ground = 1, push-out.
4. It places the sphere, refreshes it (vt `+200`) and searches the pool, or the whole scene with no pool.
   On a hit (shape `+0x126` bit 0x20): scene = 1, then for each of the record's contacts (count re-read
   each time) the point, normal and other are copied and pushed out. With no hit it runs the sweep
   (`0x0049b9a0`): gated on `+0x24` and a probe, the segment is `pos - vel·dt·(+0x28)` to `pos`, then
   refresh and search. On a hit: scene = swept = 1, and the push-out uses the record's `+0x98`/`+0x94`
   point and normal.

The push-out `0x0049b220` is MXU's `0x001f6c40`, with only the offsets changed and one change to the
condition: the straight path runs when `+0x80 == 0 || +0x20 != 0`. The impulse `0x0049ae30` is MXU's
`0x001f6860` on the `+0x80` body. Its sin/cos is the curve table's interpolation (`*0x003eab20`, step
`0x003ea9c8`).

## How it was proved

- **Function oracle, 24 000 cases** (`the_bone_collision_is_the_originals`): half push-outs and half
  solves, on state 4's nine real parts, with every word they read drawn. The other side is drawn from
  five kinds: none, a collidable with neither, a dynamics body, a vehicle (node current or not, under a
  parent or not), and a mover of class 2 or not. The turned body is none, its own or another. The solves
  cover off, no body or disabled, no terrain, at or past the box's edge, the ground arm both ways, scene
  contacts −1..3, the swept arm and swept hits, and with and without a pool. The scene is a fake on both
  sides, logging every call; the logs must match in order. Every word of the parts, bodies, shapes and
  fakes matches, and neither side writes elsewhere (see below for the ground ring). Every one of the 20 tallied paths
  is asserted taken ≥ 20 times; before the mover-class split the fewest was 953.
- **C15 replay, both captures** (`the_bone_collisions_touch_as_the_originals`): every bone solve
  (`0x0049bb00`) is served by `bone::solve`. Every row of ride-2 (459) and flip-1 (465) is bit-exact. At
  each frame's hand-over, every part's touched, ground, scene and swept words equal the original-only
  run's. Numbers: ride-2 has 12 393 solves and first touches on row 216 (26 rows). flip-1 has 12 303
  solves and 1 118 ground arms; it first touches on row 127, the row before `SETTLES` (S278: row 128),
  with 289 touches on 76 rows. Both captures have no scene hit, no swept hit and no other side.
- `./kit audit` ok.

## Not verified / not 1:1

- **The scene search, its narrow phase and the shape calls are still the original's.** The replay's
  `OriginalScene` makes nested interpreter calls. C15 never hits the scene or a sweep, so the scene
  arms of `solve`/`sweep` are proved by the oracle with a fake scene only. Porting the scene is S279.
- **The mover, vehicle and dynamics-body paths of `moving`, and the impulse path of `push_out`, are
  proved by the oracle only.** On C15 every push-out is the ground's, with no other side.
- **The ground query's cell ring is not kept.** The original's query fills its ring (`0x00541550`) and
  the terrain's `+0x9e0`/`+0x9e4`. The port reads the map and writes neither, and the oracle skips those
  words. The ring is a cache keyed by the sample, so this is safe as long as the original's own later
  queries read the same cells. The bit-exact replays (with the original's other ground queries still
  running) show that on C15, but nowhere else.
- **The rest of the rider's world step is not ported** (the other 26 constraints, the forces, the
  integrate, the iterations). The bodies the bone collision reads still move by the interpreter. That
  is S280.
- **The probe's segment in the replay sits in a different place.** `OriginalScene::segment` writes the
  two quadwords just under the hooked call's `sp`, where the original keeps them in its own frame. Only
  the stack differs; no captured word reads it.
- **The curve-table constants** (`π`, `2π`, `1/2π`, the step and its inverse) come from `node.rs`'s
  copy, already proved there. `bone.rs` keeps its own private copy rather than making node.rs's public
  (keep changes local).
- **The terrain map is assumed to be nat05's** for C15. The oracle and both replays pass on it, so it
  is the terrain at `0x00bbd720`.

## Next

S279 and S280. With both done, the rider's world runs in the port and handling::rider's flag no longer
needs the interpreter.
