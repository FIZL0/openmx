# 1 — record sweep over nat05 (FINAL)

Task F70. 2026-11-17.

## What I did

- `atv_props::Sweeps` (crates/game-sim/src/atv_props.rs): the bike record's sweep world over the
  file-built prop world, for `atv_sweep::Sweep`. Scene meshes are `Shape::Mesh` in the prop's frame.
  Everything else (zones, the ride's spheres) is tested on its centre and reach and strikes nothing.
  A plant's sphere part is stood at the plant the same way `Rays::part` stands it (F69), then tested
  at the centre and reach the stand leaves.
- Test `the_bikes_record_sweep_searches_nat05_as_the_original` in
  crates/game-sim/tests/mxvatv_bike_props.rs. The record's sweep probe (bike `+0xc8` → record
  `+0x204`) is now in the shared `data()` setup: it is added to the port's world in the gather
  probe's space and kept off the retired list.
- Nothing taken from ../opentennis (it has no ATV sweep).

## What the original does

- Set: `./kit fn 0x1b9290` (the points at record `+0xc0`, no nodes). Place: `./kit fn 0x114360`.
- Gather of the record's list (record `+0xfc`) round the probe: `./kit fn 0x1d6a60`, kind 3.
- Nearby search: `./kit fn 0x1151c0`, with the sweep's clear and pair (F46c, F51).

## How it was proved

- 3000 generated segments on state 4 (env `F70_SWEEPS` changes the count). Two in three cross a
  triangle of one of Nat05's meshes; one in three land anywhere within 60 or 400 ft of the bike. One
  in eight has a stray `w`. One in four lowers `NO_GROUPS`, so plants get searched.
- Each sweep runs once in the interpreter (set, place, gather, search) and once in the port
  (`atv_sweep::set`, `atv_world::gather` with the list's header read from the image,
  `atv_search::search` with `Sweep` over `Sweeps`).
- Compared word for word on every sweep: the box (centre, reach), the gathered list (each entry and
  its distance), the hit bit, the record (head quads, nearest normal, point and push, tally, nearest,
  part, the segment's normal, point, owner, hit, fraction, word and struck), the world form (length,
  start, direction, centre, radius) and the probe's reach.
- Last run: 1010 hit (216 distinct meshes struck), 1990 missed, 423 lists with plants searched.
- A marker (0.125) goes in the segment's fraction before each search. Only a pair whose spheres
  meet resets it, so a sphere that is reached but strikes nothing still shows. 93 sweeps reached a
  plant's sphere this way; 856 reached something without a hit.
- Mutation check: zeroing the stood sphere's reach in `Sweeps::target` failed the test (sweep 1702,
  fraction -1 vs 0.125). Without the marker that mutation had survived.
- Reading stale record words needed raw quads: `Vf::new` flushes denormals, and the image's old
  normal and point words hold some (0x100, 0x2c).

## Not verified / not 1:1

- A trunk part (a plant part that is a mesh): the sweep panics on one, because where the stand puts
  it as a collidable (`+0x50`, `+0x70`) is not ported. Nat05's plants have none, so nothing here
  reaches that path.
- Hierarchy (kind 1) and finite-plane (kind 8) collidables: the port's world has neither, so
  `Sweeps` never hands one to the pair. Nat05's scene meshes are all kind 0.
- Listeners and callbacks: `Sweeps` hears nothing. Only the record and the hit bit are compared, not
  what the original's listeners wrote to RAM. A sweep never strikes a zone (its pair strikes only
  meshes), so no zone callback can run.
- Zones and the ride's spheres being reached is only covered by the marker count ("reached, no
  hit"). It does not say which kind was reached, and no mutation was run on the zone path.
- The ride's own collidables (the vehicles' and riders' spheres, the parked machines) are retired
  on both sides, as in the ray tests, so the sweep sees the world alone.

## Next

- Nothing for F70.
