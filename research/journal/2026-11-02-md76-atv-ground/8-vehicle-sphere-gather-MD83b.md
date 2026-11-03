# MD83b — a vehicle's own sphere gather

2026-11-03. Brief: port the gather each vehicle runs round its own sphere (vehicle `+0xd80`, its list
`+0xd84`), with the 10 ft rule that files the sphere again, so `atv_world` files it when the original
does. Prove it on MD37a's images with `tests/atv_gather.rs`.

Read with `./kit dis -p atv`. The field map is in oracle.txt's MD83b block.

## What the two functions do

`atv_vehicle_sphere_place(vehicle, point, toward, out, gather)`:

1. Copies `point`, all four lanes, into the sphere's `+0x160`. The sweep's first point (`*(+0x148)`)
   becomes `point + (0, 10, 0)`. The `add.s` is `10 + y`.
2. Sets the reach (`+0x170`): 5 with the rider off (`+0x684`'s `+0x58` is 2 and `+0x680`'s `+0x40` is
   set), else 40.
3. If `+0xd38 > +0xd3c` (`c.ole.s` not taken), files the sphere again: `atv_spatial_move` with the box
   `+0x160 ± 40` and a reach of 40.
4. If `gather`, it asks the vehicle's body (`+0x3b4`) to take the point, then calls the gather.
5. Takes `abs` of `toward.y`. It also normalises `toward` with y set to 0, onto the stack, and never
   reads the result.

`atv_vehicle_sphere_gather`:

1. Clears the sweep's four per-segment buffers.
2. Copies `+0x160` and `+0x170` into the collidable's centre (`+0x50`) and reach (`+0x70`).
3. Sets `+0xd38 = |centre − +0xd90|²` (a `vsub.xyz`, then `mula`/`madda`/`madd`).
4. Only if that passes `+0xd3c` (100), sets the list's `+0x3c` (flat), runs `atv_gather_nearby(list,
   sphere, 1)` and clears flat again. Kind 1 is the static structures only. Then `+0xd90 = centre`.
5. Then it walks the list's candidates within the sphere's reach and answers whether the place is
   clear. This step is not ported (see below).

The brief's "kind 0xc4" is the list's capacity (196). The gather's kind is 1.

So the tree holds the sphere where it was put on the call after its last gather, not at `+0xd90`.
MD83's test files it at `+0xd90` and gets the same slots on all three images.

**The moving key.** The tree sub-database's pick (quadtree vt +0x20) turns a moving box into a key
without clamping it to the tree. If either side of the box, less the origin, falls outside
`0..extent`, the key is 0 and the entry is filed nowhere. A static box is clamped
(`broad_build::key`). The walk found this: a bike 39.6 ft from the low `x` edge was filed in two
nodes by the port and in none by the original. The out (vt +0x2c) also frees nodes left empty. The
port leaves them, and no query can tell the difference.

## The port

New in `game_sim::atv_world`:

- `VehicleSphere`, which holds `+0x160`, the sweep's top, `+0x170`, the list, `+0xd90`, `+0xd38` and
  `+0xd3c`.
- `Nearby::vehicle()`: 10/30, room 0xc4, filter on.
- `World::place_vehicle`, `World::gather_vehicle` and `moving_key`.
- `World::move_vehicle` now files through `moving_key`. It lost its `hi`, the terrain corner, which a
  moving box does not use.
- `add_vehicles` puts each sphere in the one space, so it can be a gather's probe.

## The proof

`a_vehicles_sphere_is_placed_and_gathered_as_the_originals` runs once on each of MD37a's three R-quad
images: `atv-r-spawn`, `atv-r-spawn-late` and `atv-r-air`. The last two are copied from s3. Each
image gets the same world `data()` builds.

Before the walks, it checks that each vehicle's sphere is in the image's slots. Then it runs walks:

- The player's quad goes through every row of the fifteen captures, then a 3,000-step random walk.
- The other five vehicles each take a 400-step walk.
- One call in 13 does not gather. On the player's quad, 17 in 97 have the rider off.

Each step runs the original's place, then `World::place_vehicle`. After every call it compares:

- the sphere's `+0x160`, sweep top, `+0x170`, and every field `same_fields` reads (centre and reach
  included);
- `+0xd90` and `+0xd38`;
- the list: every candidate with its distance, the bound, and the flat word cleared again.

After every re-filing, it compares the tree slots holding the sphere, by node path and slot. Scene
nodes' slots are not counted, because the port has none. At the end it compares the whole tree.

Each image: 8,050 places (1,054 with the rider off), 3,948 re-filings (121 with the box past the
edge), 3,659 fresh gathers, 13,324 candidates, longest list 20.

`./kit check -- -p game-sim --test atv_gather`: TESTS PASSED, 5 tests.

Hooks on the original side:

- The vehicle body's own put (`+0x3b4`'s), set to nothing. It is the vehicle's, not the sphere's.
- The BIOS's GetThreadId, SignalSema, WaitSema and PollSema, answered as one thread that always gets
  the semaphore. Growing a tree node allocates, and the heap locks.

`same_fields` now masks the port's flags to the low four bits as well as the image's. A probe filed in
the tree sets its own searched bit (0x10).

Hand mutations (8):

| Mutation | Result |
|---|---|
| re-file at `+0xd90` | red (slots) |
| gather not flat | red (list) |
| kind 3 | red (list) |
| no 10 ft top | red |
| rider-off reach ignored | red |
| the static key's edge checks only | red (slots) |
| `+0xd90` not kept | red |
| re-file at `>=` | green |

The `>=` survives because the squared distance is never exactly 100.

## Not ported / not verified

- **The place's last pass and its answer.** It tests each candidate within reach against the sweep
  (`0x00116a90`, with the rider off) and against the plants' parts within 20 ft, skipping
  `SOFTFOLIAGE` ones. It can lift the caller's `point.y` and write `out`. The lists the walks build
  hold up to 20 candidates, so this pass ran, but nothing compares what it did.
- **The vehicle body's put** (`+0x3b4`) is hooked.
- **Freed empty nodes.** The original frees tree nodes left empty. The port leaves them.
- **The atv_s images.** Their vehicles are not at these addresses, so they are not run.
