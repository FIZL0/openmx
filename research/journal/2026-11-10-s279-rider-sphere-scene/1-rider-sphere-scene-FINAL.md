# 1 — rider sphere scene (FINAL)

Task S279. 2026-11-10.

## What I did

- `game_sim::atv_sphere` (new): a rider part's sphere collidable, with its `place`, its `refresh` (the
  frame, then the centre and reach) and `Search`, a `world_search::Narrow` that does the sphere's own clear
  (the hit bit down and the record emptied) and hands everything else to the caller.
- `tests/mxvatv_bike_ride.rs`: `PortScene`, a `handling::bone::Scene<RiderRam>` that the port serves.
  - place: `atv_sphere::place` + `refresh` on the sphere read from RAM and written back.
  - segment: the probe's segment from F51's sweep glue (`sweep_set_in_ram` split so it takes the two points:
    `sweep_set_points_in_ram`).
  - search: a sphere goes through `atv_search::search` with `atv_sphere::Search`; a probe goes through F51's
    sweep search (`sweep_in_ram` split so its search half stands alone: `sweep_search_in_ram`).
  - search_all: a probe through F51's `sweep_all_in_ram`. A sphere is handed to the original.
  - spin and velocity are a mover's virtuals and still go to `OriginalScene`.
  - Every hand-off to the original is counted under a key starting `>`, and the test asserts there are none.
- Two tests:
  - `the_bone_collisions_scene_is_the_ports`: both C15 replays run with two interpreters side by side, one
    on `OriginalScene` and one on `PortScene`.
  - `the_rider_spheres_place_is_the_originals`: the place/refresh oracle.
- **Split decision** (an unattended run, so nobody could be asked). The task asks for the scene on a capture
  where a rider part hits something. C15 never hits, and the sphere's pair (box, sphere, hierarchy, mesh: the
  mesh arm alone is 1,100 bytes and 15 callees) is not ported. Proving a hit needs both, so I took what C15
  can prove and filed the rest:
  - S279a: port the sphere pair (vt `+0xb4`) with an oracle.
  - S279b: make a capture where a rider part hits a scene object or a sweep, and replay it on `PortScene`.
- opentennis was not needed: this is game physics, nothing it has.

## What the original does

- **Place**: `./kit fn 0x001c1e90`. The point is taken whole when given. When a radius is given, so is its
  square. The bone collision hands no radius.
- **Refresh**: sphere vt `+200` (`./kit dis 0x001c33f0`). It takes the node's world frame, or the identity
  when there is no node (`+0x190`). Then vt `+0xcc` (`./kit dis 0x001c3340`) stores the frame and computes
  the centre: `m3 + 0`, then `madd` of `m0·x`, `m1·y` and `m2·z`, keeping xyz, with `w` taken from the point.
  The reach is set to the radius.
- **Clear**: vt `+0xb0` (`./kit dis 0x001c1ed0`) lowers `+0x126` bit `0x20`, makes `+0x7c` the current
  record and sets the record's count to 0.
- **Pair**: vt `+0xb4` is a `j` to `./kit fn 0x001c2ec0`, which branches on the other collidable's kind.
- **On state 4**: the nine rider parts all have sweep on. Each one's sphere has flags `0x12` or `0x52` and no
  node. Each one's probe is the bike record's sweep-probe class with flags `0x13`. The pool is a nearby list.
- Full layout, addresses and counts: `research/oracle.txt` `# S279`.

## How it was proved

- **Bit-exact replay fixture, side by side**: C15 ride-2 (459 rows) and flip-1 (465 rows).
  - Each runs in two interpreters, one with the original's scene and one with `PortScene`.
  - Every captured row matches bit for bit on both, and RAM outside the stack is equal after every frame.
  - ride-2 runs 147 places and segments and 294 searches, all over empty pools.
  - flip-1 runs 2,574 places and segments and 5,148 searches, over pools of 13 or 19 candidates.
  - Nothing was handed to the original.
- **Function oracle**: 12,000 generated place+refresh cases on the nine spheres. The floats are special
  (zeros of both signs, denormals, infinities, NaNs, large and small). Each of the four place arms runs at
  least 1,000 times. All 0x1a0 sphere bytes are equal, and the original writes nothing outside the sphere and
  the stack.
- **Mutation check**: taking `w` from the product instead of the point makes the oracle fail. The file was
  restored with cp + touch.

## Not verified / not 1:1

- **The sphere pair** (vt `+0xb4`: box, sphere/sphere, hierarchy, mesh) is still the original's, behind
  `Search::rest`. It never runs on C15, because no candidate is ever within reach. Filed as S279a.
- **No capture hits the scene**. Nothing has been checked on a frame where a rider sphere or sweep hits a
  scene object: the record's contacts, the hit bit raised, the bone collision's point and normal from the
  record. Filed as S279b.
- **The sphere's whole-scene search** (no pool) is handed to the original. C15 always has a pool, so it never
  runs, and the test asserts that no hand-off happened.
- **The refresh's node arm** (the frame from `+0x190`'s node) is not read and not ported. No rider sphere has
  a node, and `PortScene` asserts it.
- **A mover's spin and velocity** are still the original's virtuals, through `OriginalScene`. They are only
  reached after a scene hit, and C15 never hits.
- **The probe's pair and whole-scene search** are F51's, and only the arms F51 proved are trusted. On C15 the
  probe's pool search finds nothing and the whole-scene search never runs.
- **A group's part stand** (a hierarchy's parts in a search) is still the original's, through
  `PartWorldInRam`. No candidate is within reach on C15, so it never runs.
- **The refresh does not move the sphere's entry in the world's spatial database**, and neither does the
  original's. The C15 RAM compare covers this, but only for the frames C15 has.

## Next

- S279a: port the sphere pair, starting with the sphere/sphere arm (`./kit fn 0x00106f70`) and the mesh arm
  (`./kit fn 0x001095c0`), with an oracle over generated pairs.
- S279b: record a capture where a rider part hits a scene object (a fence or a rider against a prop), then
  run `the_bone_collisions_scene_is_the_ports` on it with `atv_sphere::Search` given the ported pair.
