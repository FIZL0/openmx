# MD76a1 — MC02's static collision and the quad's gather

2026-11-03. Task: `./kit plan MD76a1`.

`game_sim::atv_world` builds Desert Oasis's static collision as the original does: the scene's meshes from
`mc02.res` and the plant groups from `MC02.esb`, filed into a quadtree. It also ports the gather that refills
the quad's nearby list (quad `+0xc4`). `tests/atv_gather.rs` runs the original's gather off MD37a's
`atv-r-spawn.ee.bin`, with nothing hooked, and compares.

Addresses and field offsets are in oracle.txt's MD76a1 block.

## The inputs

- The meshes are the scene's props (`Env\mc02\MC02.scn` and its `Col\…` members), read with the existing
  `world::Prop` loader.
- The plants are `MC02.esb`. I took it out of `MXVSATV.PSR` into `context/mxvatv-ps2/md37a/` (git-ignored, 168,101
  bytes, version 2). `eco::Ecosystem` reads it.
- The tree's box is the terrain's: `-border` to `width × 256 × scale`, which is (-0, 5760) on Desert Oasis.

## What the original builds

- **The tree** is MX Unleashed's quadtree, built by `broad_build::tree_over` with a least cell of 96.
  Begin and collect match the port's `Quadtree`.
- **The meshes** are collidables with category 0x600, flags enabled + within bound, nearby, colliding with
  everything, and an empty category list in mode 1. In mode 0 the port's default would have refused all
  pairs, which first hid the palm.
- **The plants** are groups: a cell, a model and a blend. The model is the species' index *in loaded order*,
  with empty slots skipped. MC02's slot 0 is empty, so slots 1-11 are models 0-10. With the slot number the
  reaches came out wrong.
- The meshes are filed first, then the groups, so each node lists them in that order. The test checks
  this node by node.

## The gather

The gather empties the list and refills it every call:

- **The bound** is `min(c - lo x, c - lo z, hi x - c, hi z - c)`, then the min of that and
  `max(radius, extra)`, then the max of that and `least`. Every min and max is a compare-and-pick, so a
  NaN or -0 goes the original's way.
- **The query** walks the tree for the box `centre ± bound`.
- **Each candidate** goes through the pair filter (`world_search::accepts` at SLUS-21104's offsets) when
  the filter flag is set.
- **The distance** to a mesh is the 3D distance, or the flat one when the flat flag is set. In the flat
  distance, dz goes into the accumulator first.
- **A plant's distance** is from its cell. The adda/msub chain runs z first, and the reach is
  `near + blend × (far - near)/128`. A distance ≤ 0 becomes 0.
- **Groups are skipped** when `no_groups` is set.
- **A candidate is kept** if its distance is within the bound. The list stops at its capacity.

## The proof

`mc02s_tree_holds_the_originals_meshes_and_plants_in_its_order` checks:

- the tree's box, scale, cell and cell size, bit for bit;
- the eco object's cell size and every used model's reach;
- in each of the 2,970 nodes, the sequence of meshes and plants against the port's. That is 23,934
  entries. The image's own moving and zone entries (268) are left out of the comparison.

It also checks every image mesh's category, flags, nearby bit, collides-with mask, categories and object
lists.

`the_quads_nearby_list_is_the_originals` runs the original's gather and the port's on 9,930 probe
centres:

- the quad's position in every row of MD37a's 15 captures (3,050);
- 24 points around each of the 120 meshes;
- 4,000 points across the map.

Every 8th generated point uses one of the other modes: filter off, flat, or no groups. For each point
it compares the bound and the list of (collidable, distance), bit for bit:

- 19,674 candidates came out equal: 9,024 meshes and 10,650 plants. The longest list is 18.
- With the filter on, the only entries the original keeps that the port does not are zones (category
  0x8000): 405 of them.
- With the filter off, the original also keeps the quad's own body (0x3012, 573), category-0 objects (5)
  and zones (30).
- In the six palm rows (race-jump.csv frames 35805 and 35851-35863), both palms (Palm_A, reach 28.6, and
  Palm_E, 23.49) are in the list.

How it was checked: `cargo test --release -p game-sim --test atv_gather -- --nocapture` (2 passed),
`./kit check -- -p game-sim --test atv_gather`, and `./kit audit`.

## Not verified / not 1:1

- **The zones are not built.** These are category 0x8000, vtables in oracle.txt. The original's filtered list
  held 405 of them across the test points, and the port's lists leave them out. The test compares only the
  entries the port builds.
- **The moving list is not built.** This covers the quad's own collidables, the riders' parts, category-0
  objects and other vehicles. The test's probe stands in the image's own object list for the filter, so the
  filter itself is compared, but nothing moving is ever in the port's tree.
- **The tree's other entries are not compared.** 239 class-7 entries are in the tree. The original files
  Zone_Platform_Machine.col's mesh a second time in two nodes, and the port does not. The second filing
  sits behind a zone at the same place, so it does not change which meshes a gather finds or in what
  order. The test keeps each collidable's first entry per node.
- **Plants are not stood on the ground.** y is not used by the tree's key or by the gather, so it was not
  checked.
- **A non-zero border is untested.** Desert Oasis's is 0.
- **No live PCSX2 run.** All of it runs off one image (atv-r-spawn). In the capture rows, the probe's
  centre is set to the quad's captured position, not to the probe's own centre during the ride.
- **Nothing in the shell or `quad` uses the module yet.** The search over the list is MD76a2.
