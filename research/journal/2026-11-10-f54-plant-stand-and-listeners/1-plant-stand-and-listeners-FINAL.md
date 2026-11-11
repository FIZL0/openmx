# 1 — plant stand and listeners (FINAL)

Task F54. 2026-11-10.

## What I did

- `crates/game-sim/src/atv_plant.rs` (new): `stand` stands a plant group's part at its plant. A sphere
  gets its size and lean scales and its frame's position, then its stand slot runs, which for a sphere is
  `atv_sphere::refresh` at its own frame. A mesh's last stood frame gets the plant's position and, when the
  plant is turned, the turn's rotation; its stand slot is left to the caller (`Stood::Mesh(frame)`). Also
  `axis_angle` (a quaternion as an angle and an axis) and `coarse` (the curve table's cosine and sine below
  an angle, not interpolated).
- `crates/game-sim/src/atv_hear.rs` (new): the riding part's two listeners. `bike_hears` is the bike's own:
  it checks the flag bit and the category mask. `kind`, `push` and `entry` are the pair log's: the kind by
  the two masks, then the push, growing the log by its step through a resize the caller supplies.
- `handling::node::turn` is now `pub(crate)` (one word), so the mesh turn reuses it.
- `tests/mxvatv_bike_ride.rs`:
  - `PartWorldInRam::part` now uses `plant_stand_in_ram` (Rust; counted as `record search part stand`,
    with no `> `).
  - `PartWorldInRam::heard_hit` sends the two listener functions to `bike_heard_in_ram` and
    `pair_log_heard_in_ram`. Any other listener is still handed on.
  - The every-port-at-once replays now assert that nothing is handed on by name. The three F46d replays
    assert 141 / 141 / 432 (ride-2) and 55 / 55 / 161 (flip-1) through `heard_in_rust`.
  - Four new oracles (below).
- research/oracle.txt `# F54`: the field map and name lines.

## What the original does

See research/oracle.txt `# F54` (`./kit fn 0x0046d3c0 -o atv`, `./kit fn 0x00493cf0 -o atv`, `./kit fn
0x0025a350 -o atv`).

**The stand.** A hidden plant (flag 0x08) or an empty slot stands nothing. The position is the three
halfwords times the eco's unit.
- A sphere: size scale `(s0 + size·(s1−s0)/128)/s0`, lean scale `(l0 + lean·(l0−l1)/128)/l0` (note the
  lean's operands are reversed), its frame's row 3 set to the position, then its stand slot at its own
  frame.
- A mesh: its frame from +0x1b0 with the position. If turned, the quaternion becomes an angle and an axis
  (the half angle from the acos table; the axis is the vector over the sine of the half, taken off the
  curve table's entry below with no correction). The angle's cosine and sine come the same coarse way, and
  the rows are turned about the axis (node::turn × rows, position kept). Then its stand slot at that frame.
- Any other kind: nothing more.

**The listeners.**
- The bike's: with flag bit 0x02 up and the struck's category within its +0xe8 object's mask, it calls the
  bike's vt +0x138. **a1 is the struck's category**, not the probe: the listener's `lw $5` overwrites it.
  The oracle caught this. The F46d hand-off passed the probe; that path is never reached on C15.
- The pair log's: kind 1, 2 or 0 by the owner's two masks. `{probe, struck, 0, kind}` is pushed. When the
  log is full it grows to count + step if that is above the capacity and no more than its most (signed);
  otherwise the pair is dropped.

## How it was proved

- C15 replays in lockstep with `posts_frame`'s handing run, RAM alike after every frame:
  - `ride_replays_with_the_part_search_in_rust`, `flip_replays_with_the_part_search_in_rust`,
    `ride_replays_with_the_searches_and_the_database_in_rust`.
  - `ride_replays_with_every_port_at_once`, `flip_replays_with_every_port_at_once`. These now hand nothing
    on by name.
- Function oracles in tests/mxvatv_bike_ride.rs, each comparing every byte of RAM, the scratchpad and the
  hooked calls (`f47_compare`):
  - `the_plant_stand_on_a_sphere_is_the_originals`: 10,000 random plants on state 4's ten species. Scale
    words include 0, −0, tiny, huge and random bits; the unit, the sphere's frame, point and radius are
    also drawn; empty and hidden slots are included.
  - `the_plant_stand_on_a_mesh_is_the_originals`: 10,000 fabricated mesh parts with a hooked stand slot
    that logs the frame it is handed. Turned and plain, other kinds, hidden; `w` is 1, −1, past ±1, ±0 or
    unit.
  - `the_plant_turns_reads_are_the_originals`: 20,000 quaternions through the axis-angle and 20,000
    angles through the coarse cosine and sine.
  - `the_riding_parts_listeners_are_the_originals`: 10,000 bike hits and 10,000 pair-log hits. The log
    covers logged (~3000), grown (~3000) and full (~4000), with the heap's resize hooked.

## Not verified / not 1:1

- **The pair log's grow still calls the original's heap resize** (an `ee.call` from the replay, counted
  under `heap: the pair log grown`, with no `> `). This follows the NodeRam heap precedent: the heap is the
  original's allocator, and C15 does grow the log (10 → 154).
- **The bike's hit slot (vt +0x138) is still handed on** when the bike hears. It is never reached on C15
  (flags 0x67, mask 0x800, category 0x80), so it shows up as no `> ` key in the replays.
- **The plant mesh is proven only on fabricated parts.** State 4 and C15 have no plant mesh (every species
  part is a sphere, eco +0x70 is 0). The mesh's stand slot is the part's own code; in the replay a mesh
  panics (`plant_stand_in_ram` passes no frame scratch).
- **A `w` with exponent 255 is not 1:1.** The PS2 FPU reads it as ±huge, while `clamp_unit` compares
  IEEE-style (a NaN is taken as above 1). PS2 arithmetic never makes such a value; one could only come from
  the eco's data. The oracle draws ±f32::MAX there instead.
- opentennis: not checked, nothing shared applies (an atv-only port).

## Next

Nothing for F54.
