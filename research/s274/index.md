# S274 — the wreck bike against the plane after C16's joust

Journal: `research/journal/2026-11-03-s274-wreck-bike-meets-the-plane`. This is the RAM side.

- **Wreck part:** `*(rider + 0x7a8)`. Its `+0x110` is the wreck bike's body and its `+0x120` the wreck
  bike's collision object.
  - At s272-137910.ram, P2's rider is 0xdee010, its part 0xdc2c50, the body 0xdc2e40 and the object
    0xdc4550.
- **Collision object:** mass at 0x64, inv_mass 0x68, cofg_world 0xa0, the touching flags 0x40..0x4c,
  the sweep at 0xd0/0xe0, the node at 0xf8 (rows +0x80, origin +0xb0), normal_speed 0x1d8,
  relative 0x1e0.
- **Masses:** the wreck collider is 3.1055897e-6 (inv 322000.03); the plane collider 310.559 (inv 0.00322).
- **Lists at 137880 and 137910:**
  - the wreck bike's: terrain, terrain, then a kind-0 entry naming the plane's object;
  - the plane's: one kind-1 mirror naming the wreck bike's object.

  The mirror at 137910: point (4567.279, 139.29706, 3515.2068), normal (0.3042866, 0.95253515,
  0.009285104).
- **P2's wreck state:** wrecked (reason 7) from row 137857 to 138012, settled from 137913, wrecked
  again (state 1) at 138191.
- **Row 137911 is torn.** It equals the plane after the first of 137910's two substeps, with the wreck
  bike's mirror on its list, to all printed digits. This holds at `frame_dt` 0x3c888882 (the image's
  `0x479cd4`), at 0x3c88ce6a and at 1/60. The tick's end is past it. With P2 standing, the substep
  misses the row. `tests/c16_fly.rs` `row_137911_is_the_wreck_bikes_first_substep`. The image's plane
  body `last_force` (+0x80) agrees with the port's resolve force.
- **`frame_dt`** is at 0x479cd4 (app + 0x344). `dt_ring` is 30 floats at 0x0046a830, its index at
  0x0046a8a8. The frames alternate 0x3c888882 (0.016666654) and 0x3c88ce6a (0.016699988).
- **C16's capture and tape are gone.** `context/capture/c16/fly.csv` and
  `context/recordings/c16/fly.p2m2` are in no slot and not in the main checkout. Every test that reads
  them skips. The s272 images remain. `segment_pad` in `tests/c16_fly.rs` rebuilds the tape's P1 pads
  from research/c16's segment list. With them, `fly_two_by` from 137845 wrecks P2 with 7 on tick
  137855, as the capture does.
- **T8: `held()` takes the first substep.** Each `Tick` keeps the plane after its tick's first substep
  (`game_sim::flight::substeps`), and a row may read that state, or be torn between it and either end.
  On C25's re-recorded capture, 137910's stretch holds 1 tick, not 0 (JOUSTS and STRETCHES[6]). It
  parts on row 137912: the port's vel is (-24.667921, 8.345952, 2.0563095) and the row's is
  (-24.762869, 8.420688, 2.1121182). C25's capture starts at row 137403, so s272-137402.ram is
  `./kit state ram` of the tape's own `fly.p2m2_SaveState.p2s` (frame_counter 137402). Every other pin
  holds unchanged on the new capture.
