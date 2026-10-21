# 1 — wreck bike collision (FINAL)

Task S69. 2026-10-21.

## What I did

- New `crates/game-sim/src/wreck_bike.rs`: `WreckBike` is the original's wreck bike. It holds the
  generic rigid body (`biplane::Body`), its mass, the part's gravity, and the collision object
  (`contact::Collider` + `contact_search::Record`, with the model's eleven rays). `WreckBike::tick` is
  the world node's update, in the original's order:
  1. clear;
  2. collide (sweep back, heightfield rays, world rays);
  3. `biplane::World { iterations 1, rate 115 }.step`. Its preforce clears the shock peak, its force
     adds gravity and resolves the contacts per substep, and its post writes the transform from
     `Body::pose`.
  - `collider()` and `record()` port the part loader's setters.
- `ride::Crash` holds the wreck bike as `bike: WreckBike` (it was `body: Chassis`). The stand-in
  `Crash::contact`/`push`/`normal_of` and the speed clamps are gone. `Crash::step` ticks the bike with
  the world's heightfield (when `CollisionEnabled`) and `NoObjects`. `origin` is the transform's
  translation. The ragdoll hand-off, the snapshot wire list, the render's wrecked pose (`last_quat`)
  and the chase camera read the new body.
- `world::World::heightfield()`:
  - `Ground` and `Place` hand the terrain object's fields (`Ground::field`);
  - `Flat` is its own heightfield;
  - any other world falls back to `Sampled`, unit cells off `ground()`.
- `tests/wreck_bike.rs`, two tests:
  - the loader against the game's own objects;
  - the five s62 wrecks replayed through the port.
- `tests/hash.rs`: the `Sim` size pin, 9736 → 10152. The fresh state's leaf count did not move.

## What the original does

The wreck bike is a physics part (`Wreckbike.phy`, read by `./kit fn 0x0015a500`) on a world node of
its own. Each frame the node's update (`./kit fn 0x0026f800`) runs these steps:

1. Clear every collision object (vt+0x74).
2. Collide each object (vt+0x7c).
3. Step the node's dynamics world (`./kit fn 0x001f29a0`, four substeps at 30 Hz):
   - preforce `./kit fn 0x0015b050`;
   - force `./kit fn 0x0015af40`: gravity, then the collision object's resolve;
   - post `./kit fn 0x0015b070`: the body's pose → `./kit fn 0x0013ebd0` → the parentless node's
     rows.

The collision object's tuning is set by the setters `./kit fn 0x0026fd80`, `0x0026ff10`,
`0x00270220`, `0x002700d0`, `0x002701d0` and `0x002701b0` (oracle.txt S69 and the bike's block at
`collision_set_mass`):

- the mass and inertia scale off the body's;
- the stiffness is |32.2·k·m|;
- the damping is 2c·√(m·K);
- the moment masks are stored by offset, not by key name.

The collidable's flags (0x9a058), the actors (8) and the collides-with mask (0x40000) are not keys;
they are read off the state. The rays are `bike\col\wreckbikem.col`'s segments.

## How it was proved

- First, the oracle. In a scratch test, calling only the node update on each s62 state
  (`context/capture/s62/<wreck>-<frame>.ee.bin`) reproduces the captured `bike_a`/`bike_b` bit for bit
  through each wreck's end (front 2602→2680, side 2607→2685, back 2477→2555, tip 2472→2550, front3 2630→2680). So the
  node update is the whole tick.
- `the_loader_builds_the_original_collision_object`: the port builds the collision object and the
  body from the disc's `Tracks\free09_ss_2.res` → `bike\wreckbike.phy` and `ride::WRECK_INERTIA`. It
  equals the state's (front-2602) on every field the loader sets:
  - body damping, angular damping, inverse inertia, centre of mass and inverse mass;
  - collider mass, inverse mass, inverse inertia, springs, masks, frictions, surfaces and flags;
  - the record's switches, rest, offset, lead, push-out, facing and capacity;
  - the ray probe, against RAM and against the `.col` file;
  - the terrain object's min, max x/z, per-sample, height scale, coarse and split, against
    `Ground::field()` of the disc's free09.
- `the_wreck_bikes_replay_bit_exact`: each wreck starts from its first state. The tuning comes from
  the port's loader and the moving fields from RAM (body, `cofg_world`, `normal_speed`, `relative`,
  flags, swept segment, node). The port then ticks with the capture's `dt` over `Ground::field()` and
  `NoObjects`. Every body field of every tick equals the capture's `bike_a[0x200..0x400]`, 362 ticks
  in all. The count is asserted; a deliberate 363 failed with "left: 362", so the replay really ran.

## Not verified / not 1:1

- **World objects**:
  - The ride hands the search `NoObjects`, so the wreck bike never hits a rock, a fence or a tree.
    The port has no world-object search over a scene's props yet, and none of the five captures
    touches an object (every contact was terrain). Follow-up S76.
  - The bike against another collision object (the rider's ragdoll, another bike) is not wired either.
    S62 does that and needs this body as the moving collider.
- **Rest**: every captured wreck ends (the reset) before the bike stops, so "until it rests" is
  "until the wreck ends". `Crash::resting`'s thresholds are not tested against the original.
- **The entry**:
  - `Crash::begin` builds the body from the chassis: position = transform of the part's CofG,
    the ride's quaternion, the chassis velocity, the spin or angular velocity.
  - The tests start from the state's body, not from the port's entry, so the hand-over from the
    riding bike to the wreck body is not proven bit-exact.
  - The body's `last_*` and accumulator fields start at zero (w 1), an assumption.
- **COLLIDABLE / ACTORS / COLLIDES_WITH** are read off the running game, not derived from a key.
- **The ray constants** are shipped as bits (the ride has no model library). They are proved equal
  to the `.col` file and to RAM.
- **Test worlds**: `world::Sampled` (the heightfield any non-`Ground`/`Flat` world gets) and `Flat`'s
  heightfield are the port's own. They cut a world's `ground()` into unit cells; no original has them.
  The existing ride tests on Yard/Flat worlds pass with them, but not 1:1 with anything.
- **The terrain's max y** (the game holds 268.6465; `Ground::field` holds 0) is not compared. The
  search reads only `x` and `z` of the box.
- **The 60 Hz shell**: the replay is at the capture's 30 Hz `dt`. At 60 Hz the world step takes two
  substeps instead of four. That is the original's own formula, but no 60 Hz capture exists.
- **`BodyAngularRateLimit` / `LimitAngularRate`**: the node update did not apply them on any captured
  tick (the replay is exact without them). The port ignores them; a faster spin than the captures
  reach is unproven.

## Next

None for S69. The follow-up is the wreck bike's world-object search (props as a
`contact_search::Search`).
