# 1 — quad record outside in Rust

Task MD118. 2026-11-12. Slot s3.

## What I did

In `sim_frame`'s `Pieces::Port` (MD105b), the record's outside calls used to run the original after
the port answered, only for their RAM side effects. Now Rust writes that RAM:

- **New `tests/atv_quad/record_outside.rs`.** It holds the bike tests' RAM glue (F46c/F46d/F51/F54),
  copied from `tests/mxvatv_bike_ride.rs`:
  - the database (`DbInRam`, `spatial_move_in_ram`);
  - the sweep's set, place and search (`sweep_in_ram`, `sweep_set_in_ram`, `sweep_answer_in_ram`);
  - the part search (`part_search_in_ram`, `part_record_in_ram`, `write_part_record`), with
    `atv_search::search` and `atv_part::PartRays`;
  - the plant groups' part stands (`plant_stand_with`);
  - the riding part's listeners (the bike's heard slot, the pair log).
  They write the collidables' searched and hit flags, the search globals, the probe's record words,
  the database move and the plant parts' frames. The quad's riding part has the same vtable and the
  same two listeners as the bike's, so the glue fits as it is. Nothing was taken from opentennis: it
  has no vehicle physics.
- **Two fixes to the copied glue, both found by the whole-RAM comparison:**
  - **The mesh pair's tree-walk setup.** On frame 81, four globals differed (the walk's mode, the
    descriptor and two stack pointers). A temporary `Ee::write` watch found the writer: the
    part's pair (vt `+0xb4`) on a kind-0 mesh sets up the tree walk before it walks. Now
    `PartWorldInRam::shape` writes them for a mesh: mode 1, the mesh's `+0x230`, and two
    stack addresses. `ram_differs` treats any stack address as equal.
  - **The trunk stand.** The trunk parts of MC02's species 7 and 8 are kind-0 meshes. The bike glue
    handed their stand slot to the original, and the glue's assert ("the frame goes in scratch")
    fired. That slot is `game_sim::atv_sync::stand` for `Kind::Mesh`: the old stood frame moves to
    before, it takes the new one, and the frame, centre and reach follow. The glue now reads the part
    with `image::collidable`, stands it in Rust, and writes `+0x130`, `+0x1b0`, `+0x1f0`, `+0x50`
    and `+0x70`.
- **`sim_frame.rs`, `Pieces::Port`:**
  - `sweep`: `record_outside::sweep_in_ram` over the record's nearby list. It asserts that its hit
    equals the probe's hit bit, and checks the answer against the port's `WorldStep::sweep` as
    before.
  - `search`: `record_outside::part_search_in_ram`. Its contacts are mapped to `Hit` and must equal
    the port's `WorldStep::search` bit for bit.
  - `node_place`: `WorldStep::node_point` runs the port's `Scene::tip(0, v)`. It then writes back
    every node's world rows (`+0x70..+0xac`) and flags byte (`+0x100`) that changed. The original's
    tip no longer runs. `WorldStep` keeps the scene's node addresses for this.
  - The modes without a port still call the original, as before.
  - Each piece of glue is counted in `WorldStep::outside`. The palm test asserts that nothing was
    handed on ("> " keys), and that the ride reached the part stand, the kind-0 part pair and the
    sweep pair.

## What the original does (palm ride, 300 frames)

Here is the outside tally from the port's glue:

| Call | Count |
|---|---|
| part pair, kind 0 (prop meshes and trunks) | 277 |
| part pair, kind 5 (plant spheres) | 133 |
| plant part stands | 149 |
| sweep pairs, kind 0 | 239 |
| bike heard slot, pair log heard | 27 each |
| pair log grown (heap resize, still the original's) | 2 |

The bike's hit slot was never called. No sweep hit, so the part place never ran (see below).

## How it was proved

Ran `./kit check -- -p game-sim --test atv_quad -- sim_frame`: all 37 tests pass. That includes
`the_palm_strikes_contacts_push_the_ports_quad`, where the 300-frame ride must equal the original's
whole frame on all of RAM outside the stack, every frame. The other modes (`Pieces::Original`, the
pieces runs, the mutation test) are unchanged and pass too.

## Not verified / not 1:1

- **The part place after a sweep hit is still the original's** (the probe's place call in
  `place_part`). No sweep hits on the palm ride, so it never runs, and its Rust is not written.
  Filed as MD138.
- **The pair log's heap resize is still the original's call.** It runs twice on this ride. Filed as
  MD135.
- **The tree-walk globals are written on every kind-0 part pair.** The watch saw the original write
  them only once in the ride (frame 81), but the port's pair meets a kind-0 mesh 277 times. RAM
  still matches each frame, perhaps because the values are the same each time (the same mesh, and
  stack addresses are equal by rule). Why the original writes only once is not found. Filed as MD137.
- **The two stack-pointer globals are "a stack address"**, not the original's exact frame offset. The
  comparison treats any stack address as equal.
- **The bike's hit slot was never reached** on this ride. Its glue is copied but not proved here.
- **The sweep's pair is still MX Unleashed's `sweep_probe::pair`** (MD119). Only misses are proved.
- **`record_outside.rs` duplicates the bike glue.** The bike tests do not have MD118's two fixes.
  Filed as MD136.
- **The interpreter is the oracle, not PCSX2.** Only this ride is proved: the atv_ quad on Desert
  Oasis's palm.

## Next

MD138 (the part place, with MD119's hitting ride), MD135, MD136 and MD137.
