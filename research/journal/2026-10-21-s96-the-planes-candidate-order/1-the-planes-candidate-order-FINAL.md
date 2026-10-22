# 1 — the plane's candidate order (FINAL)

Task S96. 2026-10-21.

## What I did

- **`crates/game-sim/src/broad_build.rs`** (new): the static tree's build, ported line for line.
  - `tree_over` is the tree's constructor: scale, levels and the smallest cell.
  - `key` is a box's key: level, corner bits and corner cell.
  - `insert` handles corners, the descent, node creation, the first empty slot or a new chunk.
  - `mesh_box` is a mesh collidable's box on the VU accumulator chain.
- **`flight_search::world`**: builds the plane's world as the original holds it.
  - One space and one table.
  - The tree is laid over the terrain field's box (least node width 96) and holds the props in scene
    order: category 0x220, flags ENABLED|WITHIN_BOUND, at their box's centre.
  - The moving list (block 1024) holds the two probes.
  - A world with no real field (the `Flat` test ground, ±f32::MAX) uses the props' box instead.
- **`Flight::nearby`**: the plane's nearby search (least 10, extra 30, capacity 128), kept across frames
  and in the snapshot and the hash. `Scene::new` gathers it once a frame around the ray probe's current
  reach (`broad_phase::gather`, kind 3), so a probe still inside the bound keeps the list and re-measures
  it, as the original does. Both probes walk that one list; the record has one search.
- **The SCENERY category is 0x220, not S48's 0x48.** The 23 nearby 0x48 meshes S48 read off the state
  are the toys (train, dump trucks, scrapers); the scene meshes are 0x220. 0x220 does not meet the
  plane's wake mask 0x40, so a mesh never wakes the plane's body.
  `flight_collision::the_masks_are_the_originals` says so now.
- **`track.rs` `single`**: scene floats now go through `phy::number`, strtod plus the truncating
  double-to-float. The host's round-to-nearest parse put most of free06's prop positions one bit high.
- **`world.rs` `Prop::place`**: the basis is now built with `orthogonalise false` (keep_forward true),
  the scene collision reader's own flags. Orthogonalising left an upright prop's up at 0.99999994.
- **`tests/hash.rs`**: the pinned size is 11232 (the search). The leaf count is unchanged.
- **New proofs** in `tests/flight_candidates.rs`.
- **Plan**: S97 (the moving world in the plane's search), S98 (placed groups and sub tables), S99 (when
  the search refills).

## What the original does

- **The tree** is built once at level build. The level build `./kit dis 0x001a4540` calls
  `./kit fn 0x001c9d80` with a least-node width of 96.0, over the terrain field.
  - The constructor (`./kit fn 0x00195d00`) sets scale = 32768 / (float)(int) the longer extent.
  - Levels halve from 16384 cells until (int)(96·scale) >= the level's size, capped at 12.
  - free06's field is 0..6912 both ways, which gives a scale of 4.7407, 6 levels and a smallest node of
    256 cells.
- **The key** (`./kit fn 0x00196110`):
  - The box is clamped to the tree. It gets no key if it is off the tree.
  - The level is the first one the box does not fit both ways, capped at the deepest.
  - Corners are masked to that level, going up one level if a masked span still exceeds the side.
  - Corner bits: 0x10 always, 0x20 for a z spill, 0x40 for an x spill, 0x80 for both.
  - key = level | corners | z0<<5 | x0<<17.
- **Insert** (`./kit fn 0x00196320`): for each corner, descend level+1 steps from bit 0x4000 (child
  2·xbit + zbit), creating nodes on the way. The chunk add (`./kit fn 0x00196c20`) takes the first empty
  slot, else appends a chunk of 4.
- **The query order**: tree nodes as the walk meets them, then chunk by chunk and slot by slot, then the
  moving list in slot order. It is never sorted by distance. A gather keeps its list while the probe stays
  inside the bound and only re-measures (S45).
- **A mesh's centre** (`./kit dis 0x001edce0 0x001edd90`): the object matrix at +0x120 and the root node
  of the mesh at +0x234. The centre is T + r0·c.x + r1·c.y + r2·c.z on the ACC, which is not quite the
  FPU `transform_point`'s sum; the half is the sum of the |rᵢ·hᵢ|. The gather measures from that +0x40
  centre.
- **The scene's collision placement** (`./kit fn 0x001aa760`) reads Position/Look/Up through ini_quadword
  (`./kit fn 0x0014b810`). Each lane is strtok, strtod, then 0x00418700, the truncating double-to-float.
  It calls mat4_basis with orthogonalise 0 and keep_forward 1 (`./kit dis 0x001aa800 0x001aa860`, a4 = 0,
  a5 = 1).
- **The save state's 34** are all on the moving list, in its slot order:
  - 23 toys, category 0x48;
  - 3 bikes, one of which has since left the world, its slot 26 now a free link (bit 1 set);
  - 8 machine race gates, category 0x200020.
  No scene mesh is within the bound there: the plane is over open ground.

## How it was proved

`tests/flight_candidates.rs`, against `biplane.p2m2_SaveState.eeMemory.bin` and free06 off the disc.
`./kit check` passes (whole gate). The three new tests ran (their counts printed in the log, quoted below).

- **`the_props_tree_is_the_originals`**: the port's tree over free06's 353 props, against the state's.
  - It matches on the origin, far corner, extent, smallest side, scale and the box-reporting flag.
  - Every state mesh of category 0x220 is matched to a port prop by the bits of its centre and radius:
    353 of 353.
  - Each prop's class, flags (low four bits), nearby bit, mask and lists match.
  - Node for node, by path from the root, every node holds the same props in the same slot order: "353
    props in 769 node entries over 416 nodes, all where the original put them".
- **`the_gather_meets_the_props_in_the_originals_order`**: 3000 cases, each at a random point near a
  random prop, some with a random radius and least/extra.
  - The original's `collision_search_gather` (`ee.call` on the state's real search with its ray probe
    moved) runs against the port's `broad_phase::gather` over `flight_search::world`.
  - The original's list, filtered to scene meshes, equals the port's (same props, same order, same
    distance bits), and so do the centre and the bound.
  - Then the probe is moved within the bound and both run again: kept and re-measured.
  - Result: "5099 lists compared (2099 kept and measured again), 0 cases the original's filled, at most
    16 props in one".
- **`the_states_34_candidates_out_of_the_ports_world`**:
  - The port's world plus the state's moving list, with stand-ins for every moving object the port lacks
    (read off the state as collidables). The bike that left goes back in the first free slot.
  - Gathered at the search's centre, it has no scene mesh within the bound, and the bound is the state's
    bit for bit.
  - Re-measured at the ray probe's current centre, it gives the state's 34 objects in the state's order.
  - 29 of the distances match bit for bit. The 5 that don't are candidates 21–25 (two toys that drive,
    three bikes), each within 1%; see below.
- **The two older bugs**:
  - Before the track.rs change, 326 of the 353 centres were one bit off in y or z (the scene position
    rounded up).
  - Before the basis change, the 8 rotated props had up 0x3f7fffff against the state's 0x3f800000.
  - After both, all 353 match.
- **The biplane ratchet** still agrees through tick 22 and parts at 23, as S48 left it. To confirm the
  verdict really ran, I copied s1's `context/capture/acceptance/biplane.csv` into this slot's context,
  then raised `BIPLANE_AGREES_THROUGH` for one run to read it. That raise was put back.

## Not verified / not 1:1

- **The 5 moving candidates' distances** (two toys, three bikes) are not the state's bits. They were
  measured where those objects stood at the plane's last sort, and the state has them where they are now.
  I didn't prove the objects moved (their old positions aren't kept anywhere I found). The test only
  checks they are within 1% and that exactly those five differ.
- **The moving world isn't in the port**:
  - the toys (S81), the other bikes and the race gates;
  - class-1 hierarchy .col files, which track.rs rejects;
  - the machine race.
  So in the ride, the plane's list holds only scene meshes. The 34 are proved with stand-ins (S97).
- **Placed groups (bushes) aren't in the tree.** In the original they take gather capacity. The 3000
  cases never filled the capacity (0 full), so nothing proved here depends on them (S98).
- **The add's insert into a table's sub tables isn't ported.** The world has one table, and the state's
  plane table has no sub tables.
- **The refill clock is assumed.** The port gathers once per frame before the probes, from the ray probe's
  current reach. plane_refresh_search's caller wasn't found, so neither the order within the frame nor the
  number of gathers a frame is proved (S99).
- **The tree's origin is +0.0 against the state's −0.0.** The test treats them as equal (`F` equality),
  because they compare the same in every comparison the key makes. A −0 input box would be the only place
  they could part.
- **The query stamps are rebuilt every frame** with the world. Only a stamp equality is read, so it would
  only matter across a 16-bit wrap within one frame.
- **Flags bits 0x10/0x20/0x40/0x80 aren't compared** on the meshes. Some state meshes have them; 0x10 and
  0x20 are a search's own marks, and the search never reads 0x40 or 0x80.
- **The track.rs and world.rs fixes reach the bike too.** Every free track's prop positions and rotated
  props' bases change by a bit. The whole gate passes, but no bike acceptance run crosses a prop
  near enough for this to show (none of the replays moved).

## Next

S97 is the natural next one. Once S81's toys are in the sim, `the_states_34_candidates_out_of_the_ports_world`
can drop its stand-ins.
