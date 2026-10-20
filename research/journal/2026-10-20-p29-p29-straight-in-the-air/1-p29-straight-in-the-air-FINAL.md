# 1 — p29 straight in the air (FINAL)

Task P29. 2026-10-20.

## What I did

The brief's fourth candidate it was: **the shell drew a pose the solver did not produce.** The solver
flies a hands-off jump straight and lands it square. The bike *on screen* was crossed up because the
shell stepped the chassis' animation machine from a request block whose two axes were zero, and in the
air those axes are the crossup sweep the `bike_air_*` clip is played at.

How it was cornered, candidate by candidate:

1. **An angular rate handed over at takeoff.** It is there, and it is taken out.
   - On race125p, a ramp laid 20° across the run-up hands the air step a yaw rate of 0.2346 rad/s.
   - The shell's new `--trace-air` off free09's ramp (`--track free09 --trace-at 3864.2,1592.5,-132
     --trace-gas`) shows −0.139 rad/s and a crossup of 0.11 rad at the lip.
   - In both, the auto-level pulls the heading onto the velocity (+6.8° on free09) and the roll to
     level. The landing is rolled under 1° with the crossup at 0.
   - The same holds with `--trace-switch 62` (every control off from the lip) and at 60 Hz
     (`--trace-dt 0.0166667`, the shell's live step, not the trace's 1/30). So it is not dt either.
2. **`air.rs` gains still at the stand-in's zeros.** They are zero on the stand-in bike (the fixture
   `.phy` has no `[Airborne Control]`), but race125p's are all loaded:

   | Param | Value |
   |---|---|
   | AirborneControlMoment | (−400, 500, −150) |
   | AirborneDamping | 450 |
   | AirborneControlForce | 150 |
   | UnrollGain / UnrollTorque | 3.5 / 50 |
   | UnyawGain / UnyawTorque | 3.5 / 500 |
   | BrakeGyroTorque | 400 |

   Point 1 is those gains doing their job.
3. **A KeyAxis stick not brought home.**
   - `KeyAxis::step` (crates/openmx/src/pad.rs) walks home at 0.10 s, answers centre on the way, and
     then answers `None` so a pad's stick takes the axis back.
   - `Controls::from_pad` puts the stick through the player's ±0.3 dead zone before `Held` rate-limits
     it. A resting pad cannot reach the lean, and a released key is centred within 0.10 s plus the
     `.phy`'s rate limit, which is the original's own walk.
   - Nothing wrong here.
4. **The drawn pose.**
   - `Chassis::request` built the chassis' request with `phase_axis: 0`, `mix_axis: 0`.
   - The airborne row (`vehicle_anim`'s `AIR` arm) plays the pair the mix axis picks *at the phase
     axis*, and the axis lags (0.15 s, 0.125 s) ease toward those targets. So every flight drove the
     chassis to one end of `bike_air_Fwd`'s sweep, root channel and all.
   - On the ground the grounded rows name no bike clip, so nothing poses the chassis (V20). That is why
     it "leaves the lip square and comes down twisted", and is square again on the first grounded
     frame.
   - Measured on the 250sx chassis (game-render's `a_hands_off_flight_draws_the_bike_square`): the
     `Bike` node turned **61.78°** off its rest at zero axes, reached about half a second into a
     hands-off drop. At the rider's axes it turned 1.56°.

The fix is wiring:
- `Chassis::riding(&Rider)` (crates/game-render/src/vehicle.rs) is `Chassis::request` with
  `phase_axis = rider.motion.pose` and `mix_axis = rider.motion.fore_aft`. Those are the same two
  numbers `Rider` already hands the rider's body.
- The shell (crates/openmx/src/main.rs) steps the chassis from it.
- No ported arithmetic changed.
- `Chassis::request` is left as it was, with its zeros, because three existing game-render tests call
  it.

Also added:
- `--trace-air` in the shell: per frame the attitude, `ang_vel`, `air.crossup` and the held stick.
- `research/p29_air_attitude.py`: the same attitude read off a capture's `bike_xform`, per airborne run.

opentennis has nothing for this (it is the drawing of a simulation pose, not the non-simulation
structure AGENT.md sends us there for), so nothing was taken from it.

## What the original does

- The rider's body is mounted on its **bike's** request block. On the V21 save states, every ridden
  rider body's request pointer (+0x488) leads to a block whose first word is the owning object. That
  object's type word is 2 and it holds `Bike\Race250A.phy`. The biplane pilot's leads to the plane
  (type 1, `Biplane\biplane.phy`).
- The block's phase target (+0x170) on an airborne rider (state 35) read 0.4996, which is square, with
  mix 0.1237. So the bike's request carries the shown crossup and the fore-and-aft, not zeros.
- The bike chassis' own animation object was not found in RAM. Its clips belong to the motion
  driver's airborne sweep (`motion::Driver::select`'s `WHEELIE` arm, phase = shown crossup, the pair by
  fore-and-aft). That arm and the sixty-state `AIR` arm pick the same pair at the same phase for the
  same two numbers, which is why feeding them to the stance draws what the motion driver would.

## How it was proved

- **The solver**: `it_flies_straight_hands_off` (crates/game-sim/tests/ride.rs).
  - Setup: race125p.phy off the disc at the capture's dt, a 20° oblique ramp, and every control off
    from the first airborne frame.
  - Result: 63 frames up, lip yaw rate 0.2346 rad/s. It landed rolled 0.00°, with the heading turned
    −1.32° and the crossup 0.0000, and 0.83° the most roll on the way. No wreck.
  - Asserted within the original's squarest landing on recording2's straight jumps: roll 0.6°
    (tick 2651), heading 4.2° (tick 3753), read by `research/p29_air_attitude.py`.
- **The drawing**: `a_hands_off_flight_draws_the_bike_square` (crates/game-render/tests/vehicle.rs).
  - The same 81-frame hands-off drop steps two chassis, one from each request.
  - Zero axes turn the `Bike` node 61.78° (asserted > 30, so the test shows the bug it guards). The
    rider's axes turn it 1.56° (asserted < 3).
- **The request's owner**: the V21 RAM dumps, read by hand with a pointer scan (scratch script, not
  committed).
- `./kit check`: TESTS PASSED.

## Not verified / not 1:1

- **There is no hands-off jump in the original's captures.** In every non-flip airborne run of
  recording2 the human held the stick at full |x|. The test's bound is the original's squarest
  *ridden* landing, not a hands-off one. The release behaviour was only seen once: ticks 3782–3800 of
  recording2, where the yaw rate decayed −40 → −7 °/s and roll +50 → +10 °/s in ~0.6 s after letting
  go. It was not compared tick by tick with the port.
- **The bike chassis' own animation object in the original was not located.** That the chassis is
  played at the bike request's axes is inferred: the rider body is mounted on the bike's block, which
  carries a square 0.4996 in the air, and the motion driver plays `bike_air_*` at the shown crossup.
  Not read off the chassis' own object.
- **The 1.56° left at the rider's axes** is `bike_air_Mid`'s own pose at its midpoint as the port
  samples it. It was not compared against the original's drawn chassis.
- **Not seen on screen.** No screenshot of a jump in the shell before and after. The measure is the
  `Bike` node's rotation in game-render's test, which is what the shell skins with.
- **`Chassis::request` still answers zero axes.** Three game-render tests use it and were left alone.
  The shell no longer calls it.
- **The pitch rate in the air is not damped below `DEAD`** (a constant ~0.07–0.09 rad/s through every
  hands-off flight, the port's straight transliteration). Not compared against the original.
- **The free09 trace start** (`--trace-at`) is a capture position taken by hand. The port's ride up
  that ramp is not the human's.

## Next

Nothing for P29. If someone wants the hands-off flight proved against the original rather than
bounded by it, a generated `.p2m2` (memory: no pad, use a generated recording) that releases the stick
on a free09 lip would give the first hands-off capture to compare `it_flies_straight_hands_off`
against.
