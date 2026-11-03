# MD83 — MC02's zones and moving collidables

2026-11-03. Brief: the zone collidables (category 0x8000), the moving list (the quad's body and probe,
the rider parts, the category-0 objects, the other vehicles), and the second filing of the
Zone_Platform_Machine mesh. All of them go into `game_sim::atv_world`'s tree and tables. The proof is
`tests/atv_gather.rs`, comparing every entry, zones included.

## What the original files, and where it comes from

The addresses and field offsets are in oracle.txt's MD83 block.

- **Zones come from the `.toy`.** `Env\mc02\MC02.toy` is inside `mc02.res`. Its `[Triggers]` section
  lists six `[TriggerN]` sections, all with `Zone = 1`:
  - four gap spheres of r 35;
  - the Walkover Gap box (Box 10, 12.5, 67.5, turned by LookVector/UpVector);
  - the Desert Baja Race start sphere of r 22. It says `StartEnabled` twice (TRUE, then FALSE). The
    first one wins, and it is filed enabled.
- **How the zones are filed.** They go in the tree after the plants, in trigger order, each boxed the
  way a mesh is:
  - a sphere: its position through the identity, with the radius each way;
  - the box: its origin through its frame, with its half sides.
  - The box frame is `math::basis(Position, Look, Up, false, false)`, bit for bit.
  - Its reach is `sqrt.s(mula, madda, madd)` of the half sides.
- **The second filing.** Next, each scene `COL=` with `Type=StartZone` is filed again with the same
  box. On MC02 that is only `Zone_Platform_Machine.col`. That is why two nodes hold it twice.
- **The vehicles' spheres.** Vehicle `+0xd80` is a sweep-class sphere of category 0, and it is the
  probe of the vehicle's own gather list (`+0xd84`). Six vehicles have one: the player's quad,
  another quad, and four bikes.
  - Each sphere is filed at the origin, in vehicle order.
  - It moves (`atv_spatial_move`, half 40 each way from `0x00525270`) only once the vehicle has gone
    10 ft from where it last gathered.
  - The tree therefore holds the player's sphere at quad `+0xd90`, not at its current centre. Filing
    it at its current centre put it in 2 nodes instead of the image's 4. That is how this was found.
  - Its old slot in node 000000 stays empty. That is the one hole in the tree.
- **The moving list.** It is MX Unleashed's `Blocks` with its fields 8 bytes down, holding one block
  of 0x400 slots, with 128 in use:
  - slot 0: the quad's frame mesh;
  - slot 1: the riding probe;
  - slot 2: the sweep;
  - slots 3–17: free;
  - slots 18–125: six riders × nine sphere/sweep pairs;
  - slots 126–127: two category-0 sweeps, each with an owner object of its own.
  - Only the frame mesh is enabled. That explains the `(no filter, 0x3012)` entries MD76a1 had to
    leave out.
- **The 239 class-7 tree entries** are scene nodes. The gather passes over them, and the port does not
  build them.

## The port

In `atv_world`, everything after `World::new`'s meshes and plants is added by separate calls, so the
callers MD76a1 already had stay as they were:

- `zones(&Ini)`, `Zone::{centre, frame, reach, bounds, collidable}`;
- `World::file_zones`;
- `start_zones(&Scene)` and `World::file_again`;
- `World::add_vehicles` and `World::move_vehicle`;
- `World::add` and `World::put_moving`;
- the constructors `vehicle_sphere`, `rider_sphere`, `rider_sweep` and `owned_sweep`.

## The proof

`./kit check -- -p game-sim --test atv_gather`: TESTS PASSED, 4 tests.

- `mc02s_tree_holds_every_collidable_of_the_originals_in_its_order` compares every node:
  - 23,965 entries in 2,973 nodes, in order.
  - The second filing (2) is counted, and the one empty slot is in place.
  - In nodes without scene nodes, slot positions are compared exactly.
  - Every class-2 entry has the image's flags (low nibble), nearby bit, reach, category, mask,
    category list, centre and object lists. That covers 78 meshes, 5 sphere zones, the box zone and
    6 vehicle spheres.
  - Only the 239 class-7 entries are left out, and the test asserts that number.
- `mc02s_zones_are_the_toys_triggers` checks each zone's centre, reach, frame rows, local centre,
  radius and r², and its half sides, bit for bit against the image.
- `mc02s_moving_list_is_the_originals` checks the list's flags, size, blocks and last, then every
  slot:
  - each free slot is free;
  - each used slot holds the same collidable with the same fields;
  - the object lists name the same collidables.
  - It also checks the space's tables: the tree is fixed, the list is moving, and there are no
    sub-tables.
- `the_quads_nearby_list_is_the_originals`: 9,930 gathers, with nothing left out any more. The lists
  now hold zones (0x195 with the filter on, 0x1e with it off), the quad's frame mesh (0x23d, no
  filter) and the dormant vehicle spheres (5, no filter), equal to the original's.

Hand mutations (8):

| Mutation | Result |
|---|---|
| basis orthogonalising | red |
| `move_vehicle` leaving the old slot filled | red |
| zone flags without ACTIVE | red |
| rider sweep's category list | red |
| the `Zone` filter inverted | red |
| no second filing | red |
| moving `last` off by one | red |
| the box reach summed z, y, x | green |

The last one survives because MC02's one box gives the same bits either way.

`./kit audit` finds only the 4 leaks that were already there (`atv_wheel.rs:62/66`,
`plan/DONE.md:519/533`).

## Not verified / taken from the image

- **Rider part radii.** The riders' sphere radii (0.4, 0.4, 0.75, 0.75, 0.75, 0.5, 0.5, 0.75, 0.75)
  come from the image. They are 0.39999998, which looks truncated from text, but I did not find the
  file they are read from (`MXVSATV.PSR` is not extracted here).
- **Runtime state comes from the image.** That covers the parts' places, the quad's three parts' own
  fields and lists, and the player sphere's filing point (`+0xd90`). These belong to the ride, not to
  the world.
- **Slot numbers.** Which slots are free (3–17) and where the riders start are laid out by the test
  as the image has them. The port does not model the free list's history.
- **The vehicle sphere's own gather** (`+0xd84`, kind 0xc4) and its 10 ft re-file rule are described
  here, not ported.
- **The `Zone` key.** `Zone = 0` triggers are skipped. MC02 has none, so this is a guess from the key's
  name.
- **The two category-0 sweeps' owners** (owner vtable ≈ `0x003cc840`) are not identified.
