# 1 — pack plane record (FINAL)

Task MD110a1. 2026-11-18.

## What I did

- `vehicle.rs`: `mxvatv_record` (the pack's `.phy`, skin `.psg` and `…M.col` through MD110a's
  `MxvatvRecord::read`), `PackFlier::mxvatv_record`, `takes_mxvatv_record(world)` and
  `with_mxvatv_record_on(flight, record, on_mxvatv_ground)`.
- `flier_refit.rs`: `PlaneFiles` carries a pack plane's record (the disc's MXU plane has none), so the
  pause's Switch To Machine refit picks it up with the rest of the skin's files.
- `main.rs` (`flight_for`): a SLUS-21104 pack plane's flight takes the record where the load's ground
  says it is SLUS-21104 terrain (`World::mxvatv_terrain`).
- Test `a_pack_plane_takes_slus_21104s_record_only_over_its_terrain` (vehicle.rs).
- Touched `flier_refit.rs` and `main.rs` beyond the task's `vehicle.rs`: the flight is built in
  `main.rs` off `PlaneFiles`, which has no `PackFlier`; both edits are a few added lines.

## Why it is gated

With a record, the flight's resolution is the record's alone (`Flight::forces` takes `held` in place of
MX Unleashed's contacts), and the record's pass (`RecordOnPlane`) asks only the world's SLUS-21104
terrain: no props, sweeps or track meshes. Over a world without that terrain a plane carrying it would
meet nothing and fall through the ground. No shell world answers `mxvatv_terrain` today (only
game-sim's `handling::ground::Under`, a test world), so in the shell the gate is always shut and pack
planes fly exactly as before. Filed MD249 to give the shell's MX vs. ATV ground a terrain answer.

## How it was proved

- The record itself is MD110a's, proved there against biplane3's image (tests/mxvatv_biplane_replay.rs);
  this task only reads it off the pack's files, through the same reader.
- The gate: the unit test above (attached over `Under`, not over `Flat`, not without a record).

## Not verified / not 1:1

- A pack plane in the shell striking SLUS-21104 ground with its hull: never seen, because no shell world
  is SLUS-21104 terrain yet (MD249). The wiring is reached only once one is.
- That the record's pass alone is enough in a shell world (it strikes no props or track meshes): left to
  MD249.
- Online: the record is not on the snapshot wire (MD110's choice), so a joiner's copy of a pack plane
  rebuilt from a snapshot has none; unchanged here, and moot while the gate is shut.

## Next

- MD249.
