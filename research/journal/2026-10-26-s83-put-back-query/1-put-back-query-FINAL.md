# 1 — put back query (FINAL)

Task S83. 2026-10-26.

## What I did

- `crates/game-sim/src/back_search.rs` (new): the put-back's mesh query as the original asks it.
  `back_search::query` gathers the bike's own search list over the plane's broad-phase world of the
  track's props and plants (`flight_search::world_wooded`, the bundle's ray collidable flagged as the
  bike's bundle is, `broad_phase::gather`), only when the bundle's centre has moved more than ten feet
  (squared distance > 100) from where it last gathered, or the world changed under it. Then every
  listed entry inside the bundle's radius is asked, in list order: a prop by `sweep_object` (the
  original's object sweep: sphere test, the bundle taken into the prop's frame by the inverse of its
  matrix, `collide::sweep` on its mesh, the flagged probes' points and normals back to the world —
  translation first, as the VU code does it, not `vu::transform`'s order); a placed group by its cell
  within 20 ft of the centre on the ground plane. Probe 0's highest point (with its normal) is the
  answer, any other probe that met refuses, a group refuses. `Race` carries a race's checkpoints
  (refusing within sqrt(100 + r²/4), `back_search::near`) and its static-only gather — the port has no
  race yet, so the ride passes `Race::default()`.
- `Ride` keeps the query's `Cache` (where it gathered, the move it measured, the list) as
  `back_search: Option<Box<Cache>>`, made the first time a world with props or plants is asked; the
  put-back's clearance ring and its overhead probe both go through it (`ride.rs` `back_solid`). A
  world with neither (the test worlds) is asked `World::solid` / `World::crowded` as before.
  Snapshot wire and the hash walk carry the cache (walked only when `Some`), so the riders' wire grew a
  byte a rider (13367 → 13371, `tests/platforms.rs`) and `Ride` and `Sim` 8 bytes (2432 → 2440, `tests/ride.rs`; 11400 → 11408, `tests/hash.rs`);
  the state hashes did not move.
- `research/oracle.txt`: an S83 block with the float orders, the reach, the gather kind, the
  checkpoint layout and the state's world.

## What the original does

`./kit fn 0x0027df50` (put_back_solid), whole; `./kit fn 0x0011c210` (bundle_sweep_object), its mesh
path. S75 had read put_back_solid's rules; S83 took the exact float orders from the listing:

- the move: `vsub.xyz` of the bundle's centre and the cached point, then FPU `mula/madda/madd`; the
  regather test is `!(move <= 100)`, the 100 from the bike's constructor, never written again.
- each entry under the radius (`c.olt.s`, no early exit): a type-2 object flagged `+0x31 & 1` and not
  masked by `0x10000` is swept; y lifts when `!(point0.y <= y)`; a flag on probes 1.. refuses. A type-3
  group's cell times the world's cell size minus the centre, `mula dx², madd dz²`, `<= 400` refuses.
- in a race (the mode object's mode 1, its flag 1), every checkpoint node: `mula dx², madd dz²` against
  `100 + (0.25·r)·r`; the first within refuses.
- the object sweep: the matrix inverted as its transpose with translation `0 − c0·t.x − c1·t.y −
  c2·t.z`; points to the object's frame translation first (`vaddax row 3, vmaddax row 0, …`).

## How it was proved

Function oracle on S66's bike save state (`context/s66/free06.eeMemory.bin`, bike 0x00dad810), the
original's own `put_back_solid` called in the EE against `back_search::query_each` —
`crates/game-sim/tests/back_search.rs`:

- First, the worlds are matched: every one of the track's props is found in the bike's table tree by
  its place and reach, with its matrix equal to the port prop's bit for bit, and every one of its 726
  plants by its cell and size bytes.
- `the_put_back_query_is_the_originals`: **12 000** generated bundles walked about the track (short
  steps that keep the list, jumps to props and plants that regather): the overhead probe (one probe,
  radius 1000), the clearance ring (14 probes, radius 40), no probes, and random bundles of 1–14 probes
  aimed at props or anywhere. Compared bit for bit: the refusal, every object that met the bundle in
  order (which prop, probe 0's point and normal, every probe's flag), the query's y and normal, the
  move (`bike +0x13a8`), the gather point (`+0x1390`) and the whole kept list (object, distance). All
  12 000 equal; 1761 refused, 523 met a mesh (571 answers), 1350 refused by plants alone, 3814
  regathers, the list never at its capacity.
- `the_put_back_query_in_a_race_is_the_originals`: **10 000** more with the mode object set to mode 1
  with its flag, one to six made-up checkpoint nodes about the bundle in scratch memory (list off in a
  fifth of the cases), and the race object's static-only word random: 1696 near a checkpoint, 4898
  static-only. All equal.

## Not verified / not 1:1

- **The state is free09's world, not free06's.** S66 named it free06; its tree holds free09's props and
  plants (all matched). No free06 bike state was used: s3's `context/s89/free06-bike.ram` (S89) exists
  but its bike was not located here, so free06's own props are proved only through the same code path.
- **What the port's world lacks is left out**: the hooked object sweep answers "nothing met" for every
  object the port has not got (toys, bikes, the free09 utility shack's hierarchy, which `Props::load`
  does not read, and one scenery-category mesh at 0x0139dc60 that is none of free09's props). Those
  entries are dropped from the compared lists. A put-back against any of them is not ported.
- **List capacity**: the original's list holds 128 and counts those extra objects too, so on a crowded
  spot it could drop a prop the port keeps. Cases with the original's list full are skipped (none
  occurred in 22 000).
- **Checkpoints and static-only have no caller**: the port has no race; `Race` is proved against the
  original's rule only, with made-up checkpoints. With no moving objects in the port's world the
  static-only gather can only agree trivially.
- **The world stamp**: the port regathers when the world's prop/plant counts change; the original's
  list is the bike's for one track. A port-only rule, untested against the original.
- **put_back_refile** (re-filing the bundle in the tree after a regather) is not modelled: the port
  gathers from the bundle's centre directly. Equal lists over 22 000 cases say it does not matter here.
- **No live capture**: proved on one save state with generated bundles, not on a recorded put-back
  in a running game.

## Next

A race in the port would give `Race` its caller (the mode object's checkpoints and the race
object's static-only word); a free06 bike state would prove free06's own props.
