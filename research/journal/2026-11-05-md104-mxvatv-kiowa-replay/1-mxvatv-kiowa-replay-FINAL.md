# 1 — mxvatv kiowa replay (FINAL)

Task MD104. 2026-11-05.

## What I did

- **Re-recorded MD80a's two Kiowa flights.** I played `research/md80a/fly.sh` (kiowa, kiowa3) on PCSX2 copy 3,
  starting from the kept `.p2s` states (`kept-context/md80a/kiowa{,3}.p2s`, copied to `context/mxvatv-ps2/md80a/`,
  with `ram.py` making the `.ee.bin` images). I used a scratch SLUS-21104 `KIT_CONFIG` (serial SLUS-21104, the
  openmx-mods iso, vsync 0x3fa6d8).
  - kiowa: 1242 rows (frames 27761..30299).
  - kiowa3: 1309 rows (frames 30183..32951). Under the machine's load the recorder skipped some frames, so
    there are gaps, which the replay steps through.
  - Both recordings ended on the frame stall after the tape ran out. That is expected: play.sh asks for 5 fewer
    rows than the tape has, and the gaps used up the margin.
  - Copies of the csvs, `.json`s, tapes and `ticks.csv`s are in `kept-context/md80a/md104/`.
- **New test file `crates/game-sim/tests/mxvatv_kiowa_replay.rs`, with six tests:**
  - `the_interpreter_flies_kiowa_bit_exact` and `..._kiowa3_...`: SLUS-21104's whole frame in `ps2_oracle::Ee`
    off each image, using MD101's harness. Each frame gets the input step's five words from the tape tick read
    two frames before. On every row it compares the body (640 bytes), the rotor (512), the aero model (512) and
    these helicopter words:
    - the measure, +0x32c..+0x3b0;
    - the wreck words, +0xc80..+0xc90;
    - the pad, the commands and the controller loop, +0x10f0..+0x1180;
    - the touch flag, +0x1198.
  - `the_kiowa_input_step_is_the_ports`: 10,000 cases of atv_kiowa_input against `Pad::from_sticks_mxvatv`.
  - `the_kiowa_strike_matches_the_original`: 10,000 cases of the crash test's reason-0 path against
    `Crash::step_mxvatv` plus `rotor_in_ground`.
  - `the_port_flies_kiowa` and `..._kiowa3`: the port ratchet. Every 100 frames a `Kiowa`/`Ride`/`Mxvatv` is
    seeded off the interpreter's RAM, then flown through `Kiowa::tick_mxvatv` beside it. The body is compared
    after each frame, and the measure, commands, loop, rotor and aero state too (the "drift" words).
- **Port changes in `crates/game-sim/src/kiowa.rs`. All are additions or changes on the `mxvatv` path only; MX
  Unleashed's path is unchanged:**
  - `tick_mxvatv` now steps the body with `Body::integrate_mxvatv` / `end_substep_mxvatv` (the biplane's
    SLUS-21104 body step; the Kiowa's body has the same limits). Before, it used MX Unleashed's.
  - The measure goes through the new `Measure::flight_mxvatv`. It is MX Unleashed's figures, but the matrix
    comes from `atv_moto::quat_frame`, as the biplane's does. The forward and up rows are refreshed from that
    matrix.
  - New `Mxvatv::unmanned`, the forces' +0x684 gate: when set, `force_mxvatv` skips the rotor and the aero.
  - New `Pad::from_sticks_mxvatv` and `command_mxvatv` (SLUS-21104's command numbers, 0x1c..0x1f; MX
    Unleashed's are 0x1a..0x1d).
  - `crates/openmx/src/vehicle.rs` builds `Mxvatv { pedal, unmanned: false }`. The player flies it, so the gate
    is open.
- Filed **MD112** for the ground contact, which is where the ratchet stops.

## What the original does

- **The input step** (`./kit fn 0x004d1fa0 -p atv`) does nothing unless +0xbc4 is set and the player record's
  "live" call (0x00249330) returns 1.
  - climb (+0x10f0) and descend (+0x10f4) take the strengths of commands 0x1c and 0x1d, as they are.
  - The lateral and longitudinal words are −x and −y.
  - The pedal is +1 if 0x1e is held, else −1 if 0x1f is held, else 0.
  - Then the d-pad (0/1 lateral, 2/3 longitudinal) overrides the stick with ±1.
  - Every lookup is the binding lookup 0x00118e10(**0x003eab60, cmd, 2, 10, player); the strength is at
    controls + slot·0x18 + 4.
  - This is MX Unleashed's step, with the helicopter's commands numbered two higher.
- **The crash test, reason 0** (`./kit fn 0x004d5410 -p atv`, `./kit dis 0x004d5410 -p atv`):
  - The wreck check at vt+0x16c comes first, unless the reason is already 13.
  - Then the in-ground loop. Each of the rotor collision object's sphere centres goes through the measure's
    matrix at +0x370, by the same VU sequence as `rotor_in_ground`. The terrain query (0x004833c0) writes the
    ground into the point's y, and the sphere is in the ground if `!(answer ≤ y)`.
  - Then, always, the object's matrix update (vt+0xcc) and the contact search (0x001151c0(obj, *(+0xc4), 1)).
  - It wrecks, in this order of tests:
    1. a sphere is in the ground;
    2. the contact search hit a surface whose flags miss the struck mask (+0x11d0);
    3. the record is touching, its surface is null or misses the crash mask (+0x11cc), and the speed is over
       MinWreckSpeed.
  - The image's masks are 0x4 (+0x11cc) and 0x10000 (+0x11d0).
- **The forces' gate** is bit 0 of +0x50 in MXVehicle's player record at +0x684. It reads 0x5e01 in both images
  and on every kiowa3 row, so the gate is open throughout both flights.
- **The measure and the controller's forward row.** The port's measure with `quat_frame`, and the controller's
  forward sign worked from the measure's row 2 (`forward_mxvatv`, MD80c), agree word for word with the game's
  on every airborne frame. The game reads that sign through atv_node_local.

## How it was proved

- **Interpreter replay, bit-exact on every row:** kiowa 1242 of 1242 rows on their own frames; kiowa3 1301 rows
  plus 8 late rows (rows 56, 1245, 1275, 1276, 1283, 1289, 1299, 1308). A late row equals the frame after its
  own, word for word: the recorder read vsync a frame early. There are no stale rows.
- **Function oracles over 10,000 generated cases each, on the kiowa image:**
  - The input step: signed-zero, end and tiny sticks; players 0 and 1; a strength in every control slot,
    including the lookup's miss slot.
  - The strike:
    - 0 to 8 of the image's 8 rotor spheres;
    - the terrain present or absent;
    - the ground under, at, one ulp above, or above each point;
    - no hit, a hit with a null surface, or a hit on a surface whose flags are drawn against the struck mask;
    - the record touching or not, its surface null or flags drawn against the crash mask;
    - the speed below, at or above MinWreckSpeed.

    The cases split as 5967 in the ground, 870 struck, 615 too fast, and 2548 flying on. The points the game
    hands the terrain match the port's `rotor_in_ground` points bit for bit, and so does every wreck.
  - Both oracles were probed with a temporary panic to show they ran, not skipped.
- **The port ratchet** (`KIOWA_AGREES_THROUGH`, `KIOWA3_AGREES_THROUGH`):
  - Every stretch seeded in the air agrees for its full 100 frames: 9 in kiowa, 10 in kiowa3. No drift word
    (measure, commands, loop, rotor and aero state) ever differs.
  - The stretch holding each landing agrees until touchdown: 92 frames for kiowa, 91 for kiowa3. An over-pin
    showed what parts there: the record is touching, and the game's body takes the ground's push (velocity y
    −0.18 in the game against −19.4 in the port).
  - Stretches seeded on the ground part on their first frame.
- The gate: `./kit check --all` (see the done commit).

## Not verified / not 1:1

- **Ground contact is not ported.** The ratchet's `Air` surroundings resolve nothing and never touch. So:
  - the take-off from the ground and the two landings part (pinned 0 and 92/91);
  - the record's touching flag, surface and entries are not compared in flight, nor is the touchdown;
  - the body wrapper's resolve (record vt+0x88 via 0x00493f60) is not compared.

  Follow-up: MD112.
- **The strike's world queries are hooked, not run.** The oracle feeds the terrain answer, the contact search's
  hit and its surface. The port's own world side (the terrain under MC02, `Collision::rotor_struck`) is not
  compared with 0x004833c0 / 0x001151c0. Neither flight strikes or wrecks, so the replay never takes that path
  either.
- **The mask numbering is not reconciled.** SLUS-21104's masks are 0x4 and 0x10000. The port's constants
  (`kiowa::CRASH_MASK` = `flight_search::VEHICLE`, `STRUCK_MASK` = 0x40000) are MX Unleashed's numbering. The
  oracle computes `ignored`/`struck` from the image's masks. What the openmx app's MX vs. ATV world hands the
  Kiowa is left to MD112.
- **The +0x684 gate is never shut in these flights.** `unmanned` is read off RAM (always false). The skip it
  causes in `force_mxvatv` is the decompile's, and no replay exercises it.
- **atv_node_local is not compared directly.** The controller's forward sign is proved only through the whole
  replay (every airborne frame's commands match), not by its own oracle. A forward speed exactly at zero, where
  the sign is decided, is not known to have been hit.
- **The edge throw is not run.** `Ride::area` is None in the ratchet, and neither flight goes near Desert
  Oasis's edge. MD80c's throw oracle covers the arithmetic.
- **rand() and the touchdown sound are not compared.** `touchdown_sound` is false in the seed, and the port's
  `Rng` is not SLUS-21104's.
- **Rows were missed under load.** kiowa3's take skipped about 76 frames and has 8 late rows; the frames
  between are stepped but not compared.
- **The curve table is the image's.** The ratchet uses the image's (as MD80c did), not `ride::Tables`'
  `CurveTable`, so the port's own table is not checked against SLUS-21104's here.

## Next

Done. MD112 is the ground contact (the body wrapper's resolve and the record on MC02), which lets the take-off and landing stretches agree.
