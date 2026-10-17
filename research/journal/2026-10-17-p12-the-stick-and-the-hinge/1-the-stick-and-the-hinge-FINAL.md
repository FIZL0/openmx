# 1 — the stick and the hinge (FINAL)

Task P12. 2026-10-17.

## What I did

- Connected the stick to the front wheel. `mv::steer_command` was worked out every substep by
  `mv::steer_command` (move.rs) and then dropped on the floor: `ride.rs`'s `patch` reads
  `wheel.w.steer_angle`, and the only writer of that field was `wheel::steer_integrate`, which
  `wheel::integrate` runs for a balancing wheel only. The two fields are **one word** of the
  original (`wheel+0x8ac`), so the fix is two lines in the one place where the two views of a wheel
  cross — `ride.rs:307` in `Wheel::to_wheel` (`self.w.steer_angle = self.mv.steer_command`) and
  `ride.rs:323` in `Wheel::from_wheel` (`self.mv.steer_command = self.w.steer_angle`) — plus the
  doc paragraphs on both halves that say why copying it in unconditionally is the aliasing itself
  and not a choice.
- Took `#[ignore]` off P10's `it_turns_the_way_the_stick_is_pushed` in both
  `crates/game-sim/tests/ride.rs` and `crates/openmx/tests/ride.rs`, and rewrote each one's
  "why it is ignored" block as what paid for it.
- Recalibrated the stand-in fixture's steering fade: `SteerScale2` 0.25 → 0.05 in `ridden_bike()`
  in both crates. At 0.25 a quarter of the 30° lock is 7.5° at top speed, which on this bike's
  4.2 ft wheelbase asks a 32 ft radius at 75 ft/s — about five and a half g from tires whose own
  `MaxSideForce*` keys hold about four — and the turn scrubbed the bike from 74.9 to 18.5 ft/s.
  The committed `.phy` stand-in has no `SteerScale*`/`SteerSpeed*` keys at all, so these numbers
  are the fixture's invention either way (see Not verified).
- Gave `crates/openmx/tests/ride.rs`'s `ridden_bike()` P8's `BodyTireForcePosition` patch
  (`0.0, 1.1, ±2.1` → `0.0, -0.3, ±0.8`), which game-sim's copy already had, and made
  `ride_the_minute()` build its rider with `Sim::with_rider` so the wheel *mounts* still come off
  the unpatched text. Without this the shell's minute steered a bike with a ±2.1 ft yaw arm and
  span 6.3 rad — a whole turn, wrapping past π, which read at the assertion as 0.0062 rad/s.
- Regenerated the four pinned constants in `crates/game-sim/tests/platforms.rs` that the new state
  word moves.

## What the original does

Three questions, three answers, all out of the decompilation.

**Where the command meets the hinge.** `./kit fn 0x00257a00` (the bike's ground step) ends its
riding pose with a single store:

    front_wheel+0x8ac = front_wheel+0x7c * bike+0x670 * fade

`+0x7c` is the front wheel's steering lock, `+0x670` the rider's lean, and `fade` the speed
interpolation between `bike+0x67c`/`+0x684` (`SteerScale1`/`SteerScale2`) over
`+0x678`/`+0x680` (`SteerSpeed1`/`SteerSpeed2`). There is no separate command word: the pose
stores the commanded angle **straight into the hinge's own angle word**, which is the same
`+0x8ac` that the tire-force path reads and that `wheel_steer_integrate` integrates. The port's
`mv::Wheel::steer_command` and `wheel::Wheel::steer_angle` are two structs holding that one word,
which is exactly why they have to be copied each way round the ported call.

**Its rate.** `+0x8b0` is never written by the pose. It is the hinge's own state: zeroed with the
steer torque `+0x6c` by `./kit fn 0x0025d1a0` (the steered-but-not-balancing case, run every
substep), and advanced from the torque and `+0x78`/`+0x70` by `./kit fn 0x0025d1b0`. So the stick
commands an angle, never a rate.

**Whether the balancing path is the only integrator.** Yes. `0x0025d1b0` is the only writer of
`+0x8ac` under the wheel, it is reached from the wheel's substep only on the balancing branch, and
the sibling branch (`0x0025d1a0`) stops the hinge dead instead. The pose's own balancing branch
writes only `+0x3c = 1` and never touches `+0x8ac`. That is the port's `wheel::integrate`
behaviour already — the gap was never in `wheel.rs`, it was that the command never arrived.

## How it was proved

- **Function oracle, 2,000 generated states**: `the_ground_step_is_the_originals`
  (`crates/game-sim/tests/move.rs`) was already proving the pose's arithmetic — it pokes the port's
  `steer_command` into `base + 0x8ac` (move.rs:243) before each call and compares the whole snapped
  wheel back out, so the identification of the two fields with that one word is the oracle's, not
  mine. It runs here (the EE image is present in this slot; 1 passed in 3.5 s, not skipped).
  `crates/game-sim/tests/wheel.rs:125,177` reads and writes `steer_angle` at the same
  `WHEEL + 0x8ac` from the other side.
- **The acceptance recording, read for the shape of a real turn**
  (`context/capture/.../acceptance` csv, read inside a test to honour the file-size guard): the
  recording's one sustained hard turn is rows 1175..1208 — 1.1 s, speed 60.6 → 61.1 ft/s, peak yaw
  rate 2.17 rad/s, peak lateral 135.5 ft/s² (≈4.2 g), flattest `up_y` 0.886. That is the
  measurement that condemned `SteerScale2 = 0.25` (which cost 56 ft/s) rather than a tune to taste.
- **The minute, both crates**: `it_turns_the_way_the_stick_is_pushed` now holds the stick right for
  four seconds and left for four, from the same ridden state P10's other legs use. Right +0.98 rad
  of heading, left −0.97 rad, minimum speed 73.6 ft/s (game-sim) / 73.3 ft/s (shell), `up_y`
  1.000 throughout, no wreck either way.
- **The pinned vectors**: `the_canonical_run_hashes_to_its_pinned_vectors`,
  `the_final_state_dumps_to_its_pinned_text` and `a_fixture_file_is_the_same_bytes_on_every_machine`
  went red, as platforms.rs's own doc demands be explained before regenerating. A probe dump showed
  the single changed word: `front.mv.steer_command` is now `0x3038e49a`, equal to the balancing
  hinge's `front.w.steer_angle`, where before it held the pose's command and disagreed. `POS_Y`,
  `VEL_Y`, `FINAL_DUMP_LEAVES` (810) and `FINAL_DUMP_HEAD` are unchanged — no physics moved, only
  the state hash over the state that now contains the aliased word. Regenerated with
  `print_the_vectors` and spliced in the four changed constants only.
- `./kit check`: the suite passes.

## Not verified / not 1:1

- **The port corners flat, and the original leans.** Through the whole minute `up_y` stays 1.000;
  the recording's hardest turn flattens to 0.886, and its flattest upright row over 20 ft/s is
  0.700. Nothing in this task's wiring produces lean in a turn, and I did not find the original's
  path to it. If it comes from the steering geometry, it comes through
  `wheel_spin_integrate`'s hinge contribution to the chassis' angular velocity (the `+0x8c0` axis
  and `+0x8d0` frame), which is still a stand-in in `ride::spin_integrate` — see the next item.
- **The turn's cost in speed is this module's own, not the original's.** The side force that turns
  the bike here is `ride::spin_integrate`'s velocity-based stand-in; the original's tire force
  function is not ported. The recording holds 60.6 → 61.1 ft/s through 1.1 s at 4.2 g, so a turn
  in the original is nearly free; here a turn is paid for in speed, and that is what forced the
  fade's far end down.
- **`SteerScale1/2` and `SteerSpeed1/2` are the fixture's numbers, not the disc's.** The committed
  stand-in `.phy` has none of the four keys, so a fixture must patch them in or the whole product
  loads as zero. `1.0 / 0.05 / 5 / 50` are mine. The fade's far end had to come down five times
  for a four-second turn to be holdable at all, which is a statement about the item above and not
  a calibration of the original.
- **The shell's fixture works around P8's open question, and the shell does not.**
  `crates/openmx/tests/ride.rs` now patches `BodyTireForcePosition` for the force arm and reads the
  wheel mounts from the unpatched text, because in this checkout that one key still does both jobs
  (P8 left that open). The real shell takes its mounts from the model's nodes, so this is a test
  fixture's two-sources trick and not how the port is wired.
- The identification of `mv::steer_command` with `wheel::steer_angle` is proved for the *pose* by
  the oracle over 2,000 states. The round trip through a whole ported substep — pose, wheel,
  patch — is not oracle-proved end to end; no function oracle spans that call in this checkout.

## Next

Nothing for P12. For whoever takes the lean in a turn: start at `./kit fn 0x0025d1b0`'s caller and
the `+0x8c0`/`+0x8d0` hinge frame, and compare against the acceptance rows 1175..1208 cited above,
where the original holds speed at 4.2 g with `up_y` down to 0.886.
