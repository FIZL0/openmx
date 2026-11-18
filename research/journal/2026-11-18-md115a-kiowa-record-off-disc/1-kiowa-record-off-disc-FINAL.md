# 1 — kiowa record off disc (FINAL)

Task MD115a. 2026-11-18.

## What I did

- `kiowa::mxvatv_record(phy, psg, col, curve)`: the Kiowa's SLUS-21104 collision record built off the
  disc. It is MD110a's `flight::MxvatvRecord::read` on the Kiowa's files: no word differs from the
  plane's build, the class word included.
- `kiowa::WreckRotor::centres()`: the rotor's collision sphere centres, the far ends of the `[Wrecks]`
  rotor rays in the ray tree's order (`WreckRotor::probe`).
- Copied off MXVSATV.PSR into `context/mxvatv-ps2/md115a/`: `Veh\kiowa.phy` (byte-identical to md80c's),
  `Veh\Anim\kiowa{,2,3,4}.psg`, `Veh\Col\Kiowa{,2,3,4}{M,F}.col`.
- `tests/mxvatv_kiowa_replay.rs`:
  - `kiowas_record_off_the_disc_is_its_images`, `kiowa3s_record_off_the_disc_is_its_images`.
  - `kiowa_flies_on_the_discs_record`, `kiowa3_flies_on_the_discs_record`: the replay with the disc's
    record and rotor centres in place of the image's (only state words carried over).
  - `port_flies` takes an optional disc build; the ratchet print/asserts moved to `ratchet` (as the
    biplane test's).
- Shell (`crates/openmx/src/vehicle.rs`):
  - `KiowaRide::record` and `with_mxvatv_record(phy, psg, col)`; `Heli::record`.
  - `built` gives a helicopter a fresh copy only when it is MX vs. ATV's and the world has a
    `mxvatv_terrain` (MD110a1's `takes_mxvatv_record`).
  - `step_through` then runs `step_on_record`: `kiowa::OnMxvatv`'s pass, `tick_mxvatv` with
    `KiowaOnRecord` (OnMxvatv's resolve, touching, entries, strike; the wreck and put-back as
    `KiowaAround`, the put-back snapped to the record's terrain).
  - `Heli::touching` reads the record when it has one.
  - Test `an_mxvatv_helicopter_flies_slus_21104s_record_only_over_its_terrain`.
- `crates/openmx/src/flier_refit.rs` (outside the file list, three lines): `kiowa_ride` builds a pack
  Kiowa's record off the pack's `.phy`, `.psg` and `…M.col`. The Kiowa is built there, as MD110a1 found
  for the plane.
- opentennis: nothing relevant (no PS2 collision record). The build follows MD110a's plane record.

## What the original does

- The Kiowa's record (`*(heli+0xc8)`) is the plane's record class, filled by the same part reader from
  `kiowa.phy`'s `[Mass Properties]`, with the same constructor words (heightfield 1, world rays 1, rests 1.0,
  actors 0x1000, collides ~0, class 0x43804; research/oracle.txt `# MD115a`).
- Model one (kiowa) runs `KiowaM.col` (22 rays), model two (kiowa3) `Kiowa3M.col` (24 rays).
- The rotor's collision object (`*(heli+0x11c8)`) holds the eight wreck rays in the tree's order; the
  strike asks the terrain under each ray's far end.

## How it was proved

Against MD80a's save-state RAM images and MD104's captures, in `crates/game-sim/tests/mxvatv_kiowa_replay.rs`:

- Record off the disc vs. the image (both flights): every non-state record word, every resolution field
  (mass, responses, surfaces, impulse mask, inverse inertia), the held mass (body `+0x18`), the part's
  rays and the rotor's sphere centres are bit-exact. Unlike the plane (MD110a), the inverse inertia is
  bit-exact too (checked first with a zero tolerance on both, then made a plain equality).
- Flown on the disc's record: every stretch of both flights reaches the same pins as the image's record
  (all 13 / 14 stretches to their end), take-offs, kiowa's landing at 1092 with its impulse, kiowa3's at 1191.
- Mutations (each run, then restored): kiowa built with `Kiowa3M.col` fails both the record compare and
  the flight. The shell's record path switched off fails the shell test (the record's pass never ran).

## Not verified / not 1:1

- **The shell never takes the record path yet.** No shell world answers `mxvatv_terrain` (MD249, as for
  MD110a1's plane). The path is reached only in the unit test, over game-sim's `Under` with no map, where
  it meets nothing and falls. A shell Kiowa landing on SLUS-21104 ground has not been seen on screen.
- **The disc's rotor centres never strike.** Neither flight puts a sphere in the ground (as MD115 found),
  so the flights only show the centres change nothing there. They are proved by word compare with the image.
- **Thrown blades on the record path** meet the world but not the helicopter's hull, and its record's
  rays do not meet them: MX Unleashed's collision object is not searched on this path, and SLUS-21104's
  record against the blades is not ported. The original's behaviour there was not checked.
- **Seams**: `KiowaOnRecord` does not wrap the body round a seam (`OnMxvatv` has none). Whether any
  SLUS-21104 terrain world wraps is MD249's question.
- **The record across a put-back** keeps its touch state and contacts; what SLUS-21104's put-back does to
  the record was not opened (no wreck in the captures).
- **Online**: the record is not on the snapshot wire (MD110's choice for the plane); a joiner's copy of
  the helicopter has none. Moot while the gate is shut.
- **Skins kiowa2 and kiowa4** (copied) have no image, so they are not compared.

## Next

Done. MD249 opens the shell path.
