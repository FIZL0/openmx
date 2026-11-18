# 1 — keep spot (FINAL)

Task F31e. 2026-11-18.

## What I did

- Found the writer of bike +0xd38..+0xd6c. It is the run's update, run by the actor update
  (`./kit fn -o atv 0x46c710`) after the sim frame and before the capture. It calls the bike's
  keep-spot (`0x4aa460`) through the owner's vt +0x20.
- Ported it as `crates/game-sim/src/handling/spot.rs`: `frame` (the run's copies from the node, then the
  keep), `keep`, `place`, `gather`. Every word and outside call goes through the `Spots` trait.
- In `tests/mxvatv_bike_ride.rs`, the pure-Rust replays (switch "keep spot") no longer poke SPOT. After
  each frame the handing run calls the original keep-spot, the Rust run calls `spot::frame`. Both must
  equal the row's +0xd38..+0xd6c.
- The Rust run's outside calls are all ports: ground query (`atv_ground::query`), the sphere's refile
  in the quadtree (`attach::spatial_move` on RAM, F43b), the gather (`gather_in_ram`), the plant part
  (`plant_stand_in_ram`, F54) and the category bit (new `category_in_ram`, a RAM read of the name table).

## What the original does

See research/oracle.txt `# F31e`. In short: pick the spot (body's centre of gravity, unless the run or
bike holds one), put the vehicle sphere there, refile it in the quadtree when it moved past its limit,
query the ground, gather round it. Keep the spot only if nothing is in the way and the ground normal's y
is over 0.8.

Two traps found on the way:

- The sphere is filed in the world's quadtree (0x672e10), not a list. `spatial_move_in_ram` refuses
  trees; `attach::spatial_move` does them (node allocs and frees go to the heap's port).
- The plant pass asks for every part (`0x46d3c0`) before it looks at the distance, and the ask
  writes the part's sphere (pc 0x1c3388, the sphere refresh). My first port asked only within 20 ft.
  The RAM split first showed on row 42 at 0x01cc4980 and broke the frame's reads on row 53.

## How it was proved

- `flip_replays_in_pure_rust`: flip-1 bit-exact on all 465 rows, wrecked on 125, put back on 193, the
  interpreter running 0 instructions in run b, no spot poke. Each row: original's and port's spot both
  equal the capture's.
- `ride_replays_in_pure_rust` and the rest of tests/mxvatv_bike_ride.rs: 151 passed.

## Not verified / not 1:1

- The sweep test (`0x116a90`), asked only for an enabled object while the rider is off, is not ported.
  The test serves it from the interpreter; C15 never asks it (run b's count stays 0).
- The run's other words (velocity, height, clocks, body +0x80, its ground query) and its event post
  (`0x272300`) are not ported: nothing in the frame reads them. Neither is `0x46b700` (flags & 0x40000).
- The own-spot (0x200000 + 0x400000), handed-spot (0x400000) and held (+0xc10) branches, the
  rider-off reach of 5, and the kind 2 object branch are not reached on C15.
- `category_in_ram` only finds a name; the original adds a missing one to the table. "SoftFoliage" is
  always there on state 4 (asserted).
- The other replays (not pure Rust) still poke SPOT.

## Next

Nothing for F31e.
