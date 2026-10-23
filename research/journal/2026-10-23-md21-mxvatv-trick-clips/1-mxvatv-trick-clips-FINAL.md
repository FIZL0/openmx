# 1 — mxvatv trick clips (FINAL)

Task MD21. 2026-10-23.

## What I did

- **The reader** (`game-render/src/skin.rs`, `Clip::parse`) now takes v4 to v7 `.mot`, the way MX vs.
  ATV's reader does. After the version, v5+ has one more i32 (it is always 1, and it is dropped). After
  `n_rot`, v6+ has `n_third` and v4+ has `n_events`. Past the rotation channels come the third
  channels (each an i32 count, then 8 bytes a key) and the events (per event: an i32, then i32
  sections; per section a string, then an i32 k, then k string pairs). Strings are an i32 length and
  that many bytes. Both are walked and dropped. A version above 7 is refused, because MX vs. ATV's
  reader stops there. Nothing changes for v3: the disc tests and the pack's 48 v3 clips parse as before.
- **The pack survey:** `mods/tricks/mxvatv_tricks` has 171 `.mot` files: 48 v3, 12 v6 and 111 v7.
  All have position kind 0, and none has a third channel or an event, so the skipping code has run
  only on zero counts.
- **Binding:** every channel of the 44 motocross rider clips (`Rider\Anim\MX_*`) names a joint of
  MXU's `mrider.psg`, and every channel of the 13 `Veh\Anim\bike_*` clips names a node of the 250sx's
  `bike.psg`. No channel is left unbound. The joints no rider clip animates are the four hand/foot
  dummies. The bike's are the dummies, the shadow, the handles, the forks, the tyres, the particles,
  the shock and the swingarm, all of which the port drives itself. The frame check: in each MX rider
  clip's first frame, the hands and feet sit 0.15–0.32 ft from the bike's grips and pegs (Cliffhanger
  is the worst, at 0.32). MXU's own `air_Mid` is 2.40 ft on the same measure. So the clips sit on
  MXU's bike as they come, and nothing is rescaled.
- **Loading** (`game-render/src/anim.rs`, `PackClips`): the pack folder is opened as a `Res`, and
  every `.mcf`'s motions are read. For the rider, the clip of `<P>_rider_<name>` is the mcf's VUE file,
  because some names differ from their files (shoalin → `MX_Shaolin.MOT`, rigamortis →
  `MX_RigorMortis.MOT`). The fallback is `Rider\Anim\<P>_rider_<name>.mot`. For the bike, it is
  `Veh\Anim\<P>_Bike_<name>.mot`, then `Veh\Anim\bike_<name>.mot`, because the MX bike's clips carry
  no prefix. `<P>` is the pack's `riders[0].rider` (`MX`). `Body::add_pack` and `Chassis::add_pack`
  fill the clips for the extras (Trick::EXTRA+k) and for ThreeSixtyL/R.
- **Playing:** `vehicle_anim` gets states past the sixty, held in a new `Library::pack`:
  `PACK_EXTRA..PACK_EXTRA+8` for the extras, then `SPIN_LEFT` and `SPIN_RIGHT`.
  - The rider's body asks for the extra's state when it has its clip. Otherwise it asks for `SLOTS`,
    which is MD16's behaviour, the airborne pose.
  - It asks for a 360 state while `air::Spin.clip` is set and the library has that side. This arm
    sits after a gesture and before the backflip and airborne arms.
  - The chassis has `want_with_pack`, used by the shell's bikes. `Chassis::want` is unchanged.
  - A state past the sixty is reached only if the library holds a pack clip for it. With no pack,
    MXU's machine is bit for bit what it was.
  - The step's own arms check for the clip too (`pack_state`, in the rate switch and the state
    switch). A first version keyed them on the state number alone. The step's 24 000-frame oracle
    then went red: it feeds state 60 as "out of the table", where the original runs no arm.
- **missing_clips** now reports per side: the rider has all 10 of the pack's clips. The bike has 4
  of 10, with none for cliffhanger, rocksolid, stalefish, mcmetz, switchblade or nofooter. The front
  flip has no clip in either game and shows the backflip's.

## What the original does

MX vs. ATV is SLUS_211.04, with the kit at `openmx-mods-slots/s1/extracted/mxvatv/kit.toml`.

- Its `.mot` reader is `./kit fn 0x139280` (boot). MXU's is `0x142750`.
  - It reads an i32. If it is negative, the version is minus that; otherwise the file is v1.
  - From there the fields run in the order given above.
- The narrow rotation decoder is `0x134ca0` (the same code as MXU's `0x13ef20`). Its table is at
  `0x3dbde0`.
- The 360's clip: GAME_REL overlay `0x4e53d0`.
  - Past 0.6108652 rad (35°), `dir < 1` plays the clip at `+0x13bc` (string `0x5279ec`,
    "threesixtyL"). Otherwise it plays the one at `+0x13c0` (`0x5279f8`, "threesixtyR").
  - It plays through vtable `+0x4b8(clip, 1)` and stops through `+0x4ac`.
- The port's `air::Spin.clip` is that state, and `dir` is that sign.
- Neither game has a front-flip clip. MX vs. ATV's front flip is a move with no animation of its
  own. So the port keeps the backflip's clip on it, as MD16 left it. It is the nearest pose in the
  rider's set, and it reads as a flip.

## How it was proved

- `crates/game-render/tests/pack_clips.rs` (5 tests, which skip with SKIPPED when there is no pack in
  `mods/tricks/`):
  - `every_pack_clip_parses`: 171 clips, versions {3: 48, 6: 12, 7: 111}. The disc's MXU clips parse
    as before (existing skin tests).
  - `the_pack_clips_bind_to_the_riders_and_the_bikes_skeletons`: no unbound channel on either
    skeleton. At least 40 rider clips are checked, and every first frame is within 0.5 ft of the grips
    and pegs.
  - `the_body_and_the_chassis_take_the_packs_clips`: the members per extra, the bike's six misses,
    and the sixty unchanged.
  - `cross_up_in_the_air_plays_9oclockindianair`: a 30 ft drop with Cross up in the air.
    - The rider spends 77 frames in state 60, and the blend plays the pack's `MX_9oclockindianair`
      clip.
    - The same ride without the pack is in the airborne state on those frames.
    - The furthest joint is 7.6 ft from the lean pose. The rider lies out to the side, which is the
      trick.
  - `no_trick_plays_no_pack_clip`.
- `crates/game-sim/tests/ride.rs`, `md21_a_three_sixty_plays_threesixty_r`: MD16's right-hand flick
  off the flip ramp, with the body run every tick.
  - While `spin.clip` is set (two windows, 89 + 8 ticks), the body asks for `SPIN_RIGHT` every tick
    and runs it, and never asks for `SPIN_LEFT`.
  - A library without pack clips never leaves the sixty.
  - One with only threesixtyL is identical to no pack.
  - Before the turn, and on the tick after it, the body asks for what it would without the pack.
  - A broken assert proved the test ran and did not skip.
- MXU replays and original-tricks rides: the full `./kit check` passes, including the replay
  hashes, rider_anim, ride and the vehicle_anim step oracle.
- A look in the shell: `openmx --track free06 --tricks mxvatv --play <tape>`, with a 30 Hz tape of
  gas plus extra 0 (R2 in the game's report, the pad's Cross under the new layout) and stick up from
  tick 258. On ticks 285 and 300 the rider hangs off the side of the bike. The plain tape sits upright
  on the same ticks. Holding the trick through the landing wrecks it, as an unfinished trick should.

## Not verified / not 1:1

- **No check by eye in PCSX2 against MX vs. ATV.** The pose is only seen in the port's shots, beside
  the plain ride, never beside the original game. MX vs. ATV is not set up to run here.
- **The narrow decoder's table at `0x3dbde0`** was not dumped and compared. The port uses MXU's table,
  on the evidence that the decoder code is the same. All 171 pack clips decode to poses on the bike
  (first frames within 0.32 ft of the grips and pegs), which a wrong table would not give.
- **v4 and v5 files were never seen.** Their layout is read off the reader, and only v3, v6 and v7
  were parsed for real.
- **Third channels and events are walked and dropped.** No pack file has one, so the skipping has
  only run on zero counts. What MX vs. ATV does with them (sounds and effects, probably) is not ported.
- **The port chose these, and the original does not say:**
  - the spin's priority (after a gesture, before the backflip and airborne arms)
  - its rate (`rate_stunt`)
  - playing it once, like the HOLD row
  - the extras' rate (`rate_stunt`)
  - MX vs. ATV runs these through its own animation system, which was not traced into.
- **The bike's prefix fallback** (`bike_<name>` for MX, whose bike clips carry no `MX_`) was picked
  from the files, not read off MX vs. ATV's loader.
- **The front flip shows the backflip's clip.** Neither game has a front-flip clip.
- **Six extras have no bike clip** (cliffhanger, rocksolid, stalefish, mcmetz, switchblade, nofooter).
  For those, the bike shows its airborne row while the rider plays the trick. MX vs. ATV may hold its
  bike still or play something else there; that was not traced.
- **The 360's clip stops when the bike comes round** (`spin.clip` back to 0) and the body goes back to
  what it asks for without the pack. Whether MX vs. ATV lets it finish (vtable `+0x4ac` is its stop)
  was not timed against the original.
- **Only the motocross rider was checked:** the anchor and binding tests use the MX clips.
  The pack's MXF and ATV clips parse, but the shell never loads them for MXU's rider.

## Next

Nothing for this task. The human may want to look at the extras beside MX vs. ATV in PCSX2 (see
Not verified).
