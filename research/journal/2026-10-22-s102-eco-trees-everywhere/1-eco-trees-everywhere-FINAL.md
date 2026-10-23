# 1 — eco trees everywhere (FINAL)

Task S102. 2026-10-22. The S97 follow-up: the wreck bike, the biplane and the camera against
`eco::Trees`, and the solid kinds other than 1.

## What I did

- `eco.rs`: each plant is now an `eco::Group` too: its cell, species, size bytes, the reach and
  half height the loader boxes it with, and its parts (one per solid: a trunk's index in
  `Trees::props`, or `None`). `Species` keeps its three heights. Kind 2 (sphere) is a part with no
  trunk, not a miss. Other kinds are still reported. `Trees` hands out `groups()`, `cell()` and
  `models()` (the broad phase's per-species reach numbers).
- `World::wooded` (world.rs, default: `collision()` and no trees). `Wooded` returns its trees there.
- `flight_search.rs`: `world_wooded` files the plants as placed groups (class `Group`, after the
  props and bikes, in the file's order) in the static tree with the loader's box, then lists every
  trunk as an object in no structure (category `TREE`). `Scene::wooded`/`refresh_wooded` are the old
  `among` ones with trees. The group's parts are reached through `Narrow::parts`/`part`, the way
  `world_search` walks groups. The ray probe meets a trunk mesh like a prop. The sweep probe's
  NO_GROUPS flag already keeps it off them. With no terrain box (none, or `Flat`'s float-wide one), the
  tree is laid over the trunks as well as the props.
- `flight.rs`: the plane's search and refresh read `world.wooded()`.
- `wreck_bike.rs`: `RAY_NODES` (the ray probe's ten tree nodes, as the state holds them),
  `ray_tree()`, and `tick_wooded`, which searches the trees through a fresh plane-style scene.
  `ride.rs`'s crash step uses it when the world has trees.
- Camera: nothing to port (below).

## What the original does

- Plants go into the world tree in the file's order (`./kit fn 0x0012b470`, key vt+0x20, insert
  vt+0x24). The box is centred on the cell × eco +0xd4, raised by half its height. It is `reach`
  wide each way and half its height up and down. reach = sp+0x10c + width·(sp+0x114 − sp+0x10c)/128,
  and half height = (sp+0x100 + height·(sp+0x100 − sp+0x104)/128)·0.5 (read in `./kit dis`: sub, mul,
  div, add, mul 0.5).
- A group's part count is the species' solid count (`./kit fn 0x00129460`). Part i is the species'
  collidable i (`./kit fn 0x00129490`), scaled to the plant: a sphere's radius and centre, kind 3's
  pair, a mesh's translation. Skipped when plant +0x12 bit 2 is set.
- A ray against a sphere (`./kit fn 0x001ebe70` case 5 → 0x00109ee0) only answers hit/miss and writes
  no contact. The port therefore gives a sphere part no trunk. The plane's and the wreck bike's
  responses read contacts only.
- Kinds on the discs (scanning every .esb): MXU has only kind 1, MX vs. ATV has 1 and 2, and Untamed
  has none of 0 or 3. "Kinds 0/2/3 for tracks that have them" is therefore kind 2, done as above.
- Camera: its collision object's nearby search (+0xe8) is never created, and its ground query
  (`./kit fn 0x001dc960`) reads only the terrain. The original's camera passes through trees, and the
  port's already does.
- Wreck bike: ray probe count +0x168 = 11, segments +0x170, root +0x160 = 0, and ten nodes at +0x16c,
  equal to `wreckbikem.col`'s. Its sweep probe's flags keep it off groups. I did not find which
  0x00218b80 caller gathers its nearby list (list in oracle.txt).

## How it was proved

- `flight_candidates.rs` `the_plants_are_in_the_props_tree_where_the_original_put_them`: free06's
  .esb from the disc, stood on free06's ground, built on the state's curve table. All 5600 plants
  sit in the biplane state's tree node for node and slot for slot among the 353 meshes (8457 node
  entries over 4530 nodes). Each matches the state's cell, size bytes and species (file slot − 1), and
  all 5600 heights equal the state's, so `Ecosystem::stand` is now proved on free06. Checked that it
  runs (forced a failure once: "5600 plants (5600 at the state's height)").
- `wreck_bike.rs` `the_ray_probes_tree_is_the_games_and_the_files`: `ray_tree()`'s nodes equal the
  s62 state's probe +0x16c bit for bit and `wreckbikem.col`'s, and its root is the state's.
- `wreck_bike.rs` `a_wreck_bike_strikes_a_free09_trunk`: the first s62 wreck with a TreeB 700 ft
  away is the bare tick to the bit for all 78 ticks. With the tree on its path, the ride parts the
  tick after the bike's centre enters the trunk's radius (25 → 26) and ends more than 3 ft nearer.
- `flight_collision.rs` `a_plane_flown_into_a_tree_crashes`: at 20 ft over a floor the plane flies
  past z 70 with no tree. With a free09 TreeB at z 60 its rays strike the trunk and it crashes
  before z 62.
- `eco.rs` `a_sphere_is_a_part_with_no_trunk_and_other_kinds_are_reported`.
- Full `./kit check` green except the known lobby LAN flake, which passed on its own rerun.
  `the_props_tree_is_the_originals`, the plane's gather/34-candidate tests and the wreck replays are
  unchanged.

## Not verified / not 1:1

- **No PCSX2 capture of the plane or a wreck bike hitting a tree.** The behaviour tests show a
  contact and a crash/deflection, not the original's tick or normal. Driving the original into a
  tree was outside the session (same reason as S97).
- **The wreck bike's nearby list is gathered fresh every tick.** The original keeps a list between
  gathers, and I did not find where it gathers it. The candidates within reach are the same, but the
  order and the timing at the list's edge may differ.
- **The wreck bike sees only trees.** The scene's props are S76's (open), so `tick_wooded` passes no
  props.
- **The wreck bike's rays catch a trunk late** (a tick after its centre is inside). That follows from
  its sweep probe not searching groups, which the flags say. Not compared with the original.
- **Sphere parts give no contact.** Only the hit bit could differ, and the port never reads it. The
  sphere's scale and placement onto the plant are not ported.
- Kinds 0 and 3 are on no disc. They are still reported, not built, and their part is `None`.
- Plant flag +0x12 bit 2 is ignored (it keeps the species' mesh where it is). It is not checked on
  free06.
- Parts are not scaled by the plant's width/height ratio for meshes (the original's kind-1 part takes
  only the translation, as S97 found; not re-checked against a plant whose sizes differ).
- `world_wooded` rebuilds 5600 groups into the tree every plane frame (as `world_among` rebuilds the
  props). It is fast enough in the tests, but not timed.
- The plane's gather with groups in it was not compared with the state's found list (the tree order
  was). The gather's group branch is `world_search`'s, proved for the bike in S75/S97.
- The shell (`crates/openmx`) still does not load trees into the ride's world, so in game nothing
  sees them yet (S97's open point).

## Next

Filed S106 (the wreck bike's gather site and clock) and S107 (a wreck into a free09 tree captured in the original). V50 (the shell loads trees) and S60 (a plane crash into an object, replayed) were already open.
