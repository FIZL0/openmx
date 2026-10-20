# 1 — p16 a flip you can do (FINAL)

Task P16. 2026-10-20.

## What I did

- **Measured the lip on P10's jump leg first, as asked.** S31 has not landed, so this is the port's number,
  not the original's. Over the minute's jump (`RAMP` = 250 ft out, slope 0.3, 15 ft long, taken at about
  70–74 ft/s), `on_a_lip`'s four conditions hold for **16 ticks, 0.267 s**: ticks 2241–2256 of the
  game-sim minute, sampled at the end of each tick.
- **The brief's diagnosis does not hold. The key ramp is not what limits the cross; the bike's own rate
  limit is.** `air.rs` reads `held.stick`, and `Held::step` walks stick y toward the hand at the `.phy`'s
  `YAxisRateLimit`.
  - On the stand-in bike that is 6 a second. −0.8 to +0.8 takes it 16 ticks (0.267 s), −1 to +0.8 takes
    18, and stop to stop takes 20, however fast the hand is.
  - On the retail 125 (`Race125P.phy`, read with a throwaway probe that was not kept) `YAxisRateLimit` is
    3.5, which makes that cross 0.46–0.51 s.
  - `KeyAxis`'s reversal (0.10 s home, 0.18 s out) stays ahead of that walk from start to end, so it adds
    nothing to a cross. Ridden in the new test, the arrow keys fire the preload and the backflip **on the
    same frames** as the stick (230 and 249 of the leg).
  - The one place the keys lose time is a stab out from centre. A stick goes there in a frame; the key
    ramp takes 0.18 s. So the stab that ends the turn passes −0.8 about eight frames later on the keys.
- **Decision: neither a faster reversal ramp nor d-pad keys.**
  - A faster ramp would change nothing the bike can feel, because the rate limit swallows it.
  - D-pad keys would not reach the flip at all. `Controls::from_pad` puts the d-pad on
    `dpad_y`/`dpad_dir`, but `Held` never feeds it into the stick y that `air.rs` reads. The disc's
    `XDPadRateLimit`/`YDPadRateLimit` keys suggest the original's binding does feed it, but that table is
    not ported. Binding keys to the d-pad today would be keys that do nothing on a lip.
- **So what gives is the ramp and the bike.** The jump's lip (16 ticks) is shorter than one rate-limited
  cross, and the pump-and-flip is three of them. Also, the stand-in bike has no `[Preload]` or
  `[Backflip]` section, so every key loads as zero. On a zero bike the gesture still fires, but with no
  torque, and the turning phase ends on its first substep. So, in both ride tests:
  - `RIDE_FLIP` is appended to `ridden_bike` the way `RIDE_LIMITS` is. The numbers are this file's, not
    the disc's: Preload 0.25 / 600 / 0..0.5 s, and Backflip 160..300 torque, +100 and −1000 a second,
    0..0.5 s.
  - `FLIP_RAMP` is (100 ft, 0.5, 25 ft). From a standstill that is about 42 ft/s at the foot and a lip of
    about 50 ticks.
- **An eleventh leg, `the flip`, 420 ticks.** It starts from the standstill `stopped` leaves:
  - flat out from tick 0;
  - stick y back at 190 (the preload arms), forward at 212 (the preload fires at 230, and the backflip
    arms), back at 232 (the backflip fires at 249, on the lip);
  - centred at 262, a forward stab at 295–305 to end the turn, then the bike comes round onto its wheels.
  - The same leg is in `crates/openmx/tests/ride-minute.pad` in pad notation (`.pad` y down positive, so
    `stick l 0 1` is pulled back), and in game-sim's `minute()` as `Leg::then` per-tick hand changes.
- **`it_flips_off_the_lip`, in both ride tests.** It checks that:
  - the lip held for more than 30 ticks;
  - the preload fired on the lip, and the backflip fired on the lip and after it;
  - the bike was more than 30 ticks in the air;
  - it came 2π ± π/2 over in the ramp's plane, and it came 1.00 turn;
  - it landed with the jump's two-part bound (up > 0.6 through the impact, > 0.9 after `SETTLE`), still
    rolling, with no wreck.

  The openmx copy rides the leg **twice**: on the script's stick, and then from a `Sim` clone at the leg's
  first frame, with the left stick's y coming from the shell's own `pad::Pads::build` with UP/DOWN held
  where the script has the stick. The two backflip frames must be within 2 of each other (measured: equal).
  To make that possible:
  - `tests/ride.rs` compiles `src/window.rs` and `src/pad.rs` in by `#[path]`.
  - `Pads::unplugged` and `Pads::build` became `pub(crate)`.
- **Release tick.** I swept it on the stick: 290 lands a quarter of a turn short, and 310–325 come down
  nose-high at up 0.3–0.4. Everything from 295 to 305 lands. The script uses 295 because at 300 the
  keyboard's later stab leaves it riding away at up 0.898, which fails the 0.9 bound.
- **Two whole-minute checks now cover the riding, not the stunt.** A new `Minute::riding()` is
  `the launch`..=`stopped`.
  - `the_minutes_numbers_are_the_originals`' slowing envelope (< 2 × the recording's p99, 70.9 ft/s²)
    uses it. The flip lands off a 12.5 ft face, and even a level touchdown loses 4–5 ft/s in a tick
    (342 ft/s² at the 295 release).
  - `the_minute_notices_a_bike_it_cannot_ride`'s "never falls over" uses it too. The slow bike reaches
    the flip's lip late and is upside down in the air when the leg ends.
  - `none_of_the_minute_wrecks` still covers all eleven legs, and still ends on the ground.
- The KEY_TO_EDGE doc in `pad.rs` now records the rate-limit finding. Nothing in `air.rs` moved.
- `./kit check`: passed (569 tests). `./kit audit`: ok.

## What the original does

Not read this task. `air.rs`'s `on_a_lip`, `preload` and `backflip` are already ported, and the brief
froze them. The only original-side facts used are the retail 125's `.phy` values above, read off the disc
for comparison and not committed. They are `YAxisRateLimit` 3.5, `[Preload]` 0.25 / 600 / 0..60, and
`[Backflip]` 800..1400, +500/−2000, 0..60.

## How it was proved

- **Bit-exact simulation tests.** `crates/game-sim/tests/ride.rs::it_flips_off_the_lip` and
  `crates/openmx/tests/ride.rs::it_flips_off_the_lip` (the latter on the pad report path and on the
  shell's keyboard `Pads`), both under `./kit check`.
- **The lip measurement and the release-tick sweep.** These came from temporary probes in the game-sim
  ride test, which were removed before commit.
- **No emulator was run.** There was nothing to compare against until S31.

## Not verified / not 1:1

- **The lip length is the port's, not the original's.** S31, which measures the lip on the original, has
  not landed. The 16 ticks over P10's jump is the port's own `on_a_lip` on the stand-in bike.
- **The lip is sampled at the end of each tick, not on each substep where `air.rs` evaluates it.** The
  test repeats `on_a_lip`'s four conditions on public fields because the function is private. The
  gestures firing "on the lip" is asserted against that sample.
- **The flip's bike numbers (`RIDE_FLIP`) and ramp (`FLIP_RAMP`) are mine.** I chose them against the
  stand-in bike's inertia so that one turn comes out. The disc's torques (800..1400) turn this bike over
  about seven times in a second of air. Whether the retail bike flips off a real lip in the time the
  stick allows is S31's question.
- **The disc's `MaxPreloadTime` and `MaxBackflipTime` are 60, and `MaxDPadPreloadTime` is 40.** Read as
  seconds against `air.rs`'s `stick_hold`, the force a fraction-of-a-second hold earns would be almost
  nothing. Either they are frames, or the hold is in other units in the original. I did not check, and
  the test bike sidesteps it with 0..0.5 s.
- **D-pad → stick y is unported.** `Held` does not feed `dpad_y` into the stick y that the preload and
  backflip read. On the disc the `XDPad`/`YDPadRateLimit` keys say the original's binding table does.
  Until it is ported, no d-pad or d-pad key can pump or flip. That is why `keyboard()` binds no d-pad
  keys yet.
- **The touchdown is harder than anything the recording covers.** The flip's landing sheds about
  342 ft/s² in a tick, against the recording's p99 of 70.9. The minute's slowing envelope now excludes
  the flip leg because of that. Nothing compares a landing with the original's.
- **The keyboard ride is the key ramp on the left stick y only.** The rest of that frame's report is the
  script's (throttle on the pressure byte, sticks). The keyboard throttle's `KeyRamp` is not part of the
  flip ride.
- **`src/pad.rs`'s and `src/window.rs`'s unit tests now also run inside the `ride` test binary**, because
  of the `#[path]` include. That is harmless duplication, and `./kit check`'s count went up because of it.
- **The "minute" is now 67 s**, 4020 frames. The name stays because the tests and the `.pad` say it.

## Next

Nothing for this task. Possible follow-ups, none of them owned:

- Port the d-pad → stick binding, then bind d-pad keys.
- Once S31 lands, re-measure the lip on the original. Then decide whether the retail bike's 3.5 a second
  rate limit and 0..60 hold times make a flip possible off the disc's own ramps.
