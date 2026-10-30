# 1 — r2 light press edge (FINAL)

Task K15. 2026-10-30.

## What I did

- Read the user's `fly_and_collide.p2m2` (PCSX2-v2.9.52, author ryha, 3,523 frames) byte by byte from the
  main checkout's `context/recordings/` (copied into this slot's `context/`, the original left as it was).
- **The byte mapping is right, and the recording does have pressures.** Every bit/pressure pair agrees
  on every frame except one R2 press: the R2 bit is down for 7 fields (file frames 1345–1351) while
  R2's pressure byte reads `00 00 00 00 00 0b 0b`. The host pad has analogue triggers: L2 takes
  0x47, 0xbd, 0xdb, 0xf0 as well as 0xff, and R2's only non-zero value is 0x0b. So PCSX2 sets the bit
  while the trigger's pressure still writes as 0: the light edge of an analogue press. The face
  buttons and shoulders are all 0/0xff.
- `P2m2::check_pressures` (crates/game-sim/src/replay.rs) now accepts "down by the bit, 0 by the
  pressure" **per control, only for a control whose pressures in that file are graded** (some
  byte strictly between 0 and 0xff). An up bit under a non-zero pressure is still refused. So is a
  down bit with no pressure on a control that is all 0/0xff, which means a rotated pairing or a pad
  with no pressures still fails. New `P2m2::light_presses()` counts the accepted fields per pad and
  control. The game reads these controls from the pressure byte only, so a light field is a released
  control to the original and to the port alike. Nothing about what the tape feeds the port changed.
- A second difference this recording brings: **PCSX2 2.9.52 wrote pad 1 too** (plugged in, untouched:
  `ffff7f7f7f7f` + zero pressures on 3,522 frames). The context test asserted `fields × 12`
  agreements. It now asserts 12 × the written (pad, field) pairs, like the 2p test already did.
- Tests: `the_pressure_bytes_of_a_recording_prove_the_byte_mapping` gained a synthetic case for each
  rule: light edge accepted and counted, no graded field means refused, and an up bit with a pressure
  is refused. `the_human_recording_in_context_drives_the_port` checks that light fields stay under 1%
  of the fields (5 of 3,523 here) and prints them.

## What the original does

Not a game-logic change. The poll reads the twelve analogue controls from the pressure bytes
(`input::CONTROL_MAP`). This task only changes which recordings the reader accepts.

## How it was proved

The byte dump of fly_and_collide.p2m2 above: one disagreement region, a single control, a graded one.
`./kit check -- -p game-sim --test replay` passed (106 tests) with fly_and_collide.p2m2, its savestate,
first_recording, recording2 and animandpump in this slot's context/recordings.

## Not verified / not 1:1

- **Why PCSX2 writes the bit before the pressure.** I did not read PCSX2 2.9.52's DualShock 2 code. My
  guess is that its button threshold and its pressure scaling/deadzone differ, but I did not check.
  The rule rests only on what this file shows.
- **Whether the original game ever reads the R2 bit for anything.** CONTROL_MAP says the twelve
  analogue controls come from the pressure byte only, and I took that from earlier work. I did not
  re-prove it against the original here.
- The 1% bound on light fields in the context test is a sanity limit I chose, not a measured property.

## Next

Nothing.
