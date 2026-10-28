# 1 — speed after the walk (FINAL)

Task S140. 2026-10-28.

## What I did

- `Ride::frame`'s step 4b (`crates/game-sim/src/ride.rs`), the attachment walk's chassis measure,
  now runs the whole `chassis::measure`: the speed (`+0x34`, the length of the velocity the substeps
  left) as well as the euler S138 ported. Before this the port's `chassis.speed` after a frame was the
  top-of-frame one, i.e. a frame stale.
- Looked for the trick request's speed read and found none in the original: stunt_request
  (`./kit fn 0x002589c0`) and stunt_idle (`./kit fn 0x00258cc0`) read no chassis field at all, and the
  stunt clocks and bike_wreck_test (`./kit dis 0x002593e0 0x00259b00`, `./kit dis 0x002522c0 0x00252c90`)
  touch the chassis (`+0x7a4`) only at +0x1a0, +0x260, +0x264, +0x294..+0x29c and a vt+0x80 call, never
  +0x34. The port's `trick::Bike::speed` feeds only `Tricks::landing`, which nothing calls (P3's
  mis-reading, documented there). So the post-walk speed reaches only what reads the bike after its
  update: in the original the chase camera's target (`rider+0x860`, `./kit dis 0x0027c580 0x0027c800`),
  the speedometer and the engine's sound; in the port `chassis.speed`'s readers after `Sim::frame` —
  the shell's camera, HUD, tire tracks and sound, `Sim::event_rider` (P155) and `markers::at_rest`
  (P141).
- Tests (`crates/game-sim/tests/replay.rs`):
  - new `the_attachment_walk_measures_the_speed_after_the_physics`: on each of the nine S67/S115 RAM
    images the original's `bike_vehicle_update` runs one tick in the oracle (dt = the state's own
    frame_dt); its saved `+0x34` is the port measure of its saved velocity, and the `+0x34` it leaves is
    the port measure of the velocity it leaves, bit for bit, and not of the velocity it began with;
  - `the_nose_first_touchdown_tick_is_the_originals` gains a `chassis speed` lane: the port's tick from
    the S115 image and pad against the original's, bit for bit (68.86976 and 57.76226 on both);
  - `save_state_watches` held the capture's `ch_speed` against the port's a tick later (shift 1) to
    absorb the stale speed; now on the same tick (shift 0), and S64's save-state run still agrees on
    every tick of every lane.
- Knock-on: `chassis.speed` is in the state hash, so the hashed vectors moved and were re-spliced from
  `print_the_vectors`/`print_the_riders_vectors` (platforms.rs: HASHES, TRACE_DIGEST, FINAL_DUMP_DIGEST,
  FIXTURE_TEXT_DIGEST, RIDERS_HASHES, RIDERS_DIGEST, RIDERS_WIRE_DIGEST), HUNDRED_TICKS in its three
  places, and MD25's two folds (tests/ride.rs). POS_Y, VEL_Y, FINAL_DUMP_HEAD, FINAL_DUMP_LEAVES and
  RIDERS_WIRE_LEN did not move: the ride itself is unchanged, only the field's value at the frame's end.
- opentennis: nothing to take (no chassis measure there).

## What the original does

The riding branch of the bike's vehicle update calls the attachment walk after the air state; the walk
calls each enabled child's vt+0x38, the chassis' being chassis_measure (`./kit fn 0x0015c4c0`), which
writes the euler (+0x90) and the speed (+0x34 = |+0x260|, `mula/madda/madd/sqrt`). The next frame's
measure takes both again before the ground step reads them, so inside the simulation the post-walk speed
has no reader; outside it, the camera target block, the speedometer and the engine sound read it through
`rider+0x860` (= `*(rider+0x7a4)+0x34`). Notes: research/oracle.txt, S140 lines under the S138 block.

## How it was proved

- Oracle over save-state RAM images: 9 images (s67/landings-2143/2745/3386/3452/3492, s67/run2-2750/2751,
  s115/landings-3457/3458), the original's whole vehicle update vs the port's measure, bit for bit.
- Port against original, one tick from the original's own state and pad: the S115 touchdown test's
  `chassis speed` lane, bit for bit on both images. With step 4b put back to the euler only, that lane
  fails by 38,658 ulp (68.57482, the frame's first speed, against 68.86976), so the test catches it.
- Capture: S64's save-state run (`the_save_state_run_starts_from_the_originals_wheels`) now compares the
  captured `ch_speed` to the port's on the same tick and agrees on every tick; with the old one-tick
  shift and the new code it parted at tick 2 (0.6058402 vs 0.60800713), which is how the shift was found.
  A scan of every s67 and s138 capture row: `ch_speed` matches its own row's |ch_vel| on all rows (the
  a few torn rows (one in s67/landings, s138/wall and s138/probe) match the previous row instead,
  within 1e-5; the scan used numpy float32 arithmetic, not the PS2's).

## Not verified / not 1:1

- The trick request's speed read the task names does not exist in the original (see above), so there
  is no capture of it to check; what was checked is the speed the port hands it (`trick::Bike::speed`,
  now the post-walk one), which only the uncalled `Tricks::landing` reads.
- That the speedometer and engine sound read `rider+0x860` after the bike's update (and not before the
  next one) rests on the oracle's earlier notes (speedo, engine sound lines), not re-traced here; the
  camera's read is the only one whose position in the frame is listed.
- `Sim::event_rider` and `markers::at_rest` now see the post-walk speed; whether the original's
  freestyle event start (P155) and the plane teleport's rest test read the chassis speed at the same
  point of its frame was not traced.
- The S115 images and capture were copied into this slot's context from s1; in a slot without them,
  the S115 halves of both tests skip with a line (the S67 images are in s3).
- The re-pinned platform vectors are this machine's only; no cross-arch run
  (memory: cross-arch runs need an upstream toolchain).

## Next

Nothing for S140.
