# 1 — car engine (FINAL)

Task S149c. 2026-10-29.

## What I did

- Ported the car's engine and automatic gearbox as `game_sim::car::Drivetrain` (crates/game-sim/src/car.rs):
  `step` (once a physics substep), `shift` (once a frame), `set_gear`, `set_reverse`, `reset`, and the
  engine's view of a driven tire (`DriveTire`) and of the car's pedals (`Pedals`). `Drivetrain::new` takes the
  loader's `Engine` (S149).
- Ported strictly from `./kit dis 0x00399e40 0x0039a790` and `./kit dis 0x00399bb0 0x00399e40`: Ghidra's C for
  both has the multiplies in the wrong association order and loses the `adda`/`madd` lerps.
- Named the functions in research/oracle.txt (S149c block): `car_engine_step`, `car_engine_shift`,
  `car_engine_set_gear`, `car_engine_set_reverse`, `car_engine_reset`, `car_tire_add_drive`, `car_shift_sound`,
  and the engine object's running words as `#` comments.
- Corrected S149a's guess (in the oracle.txt block; S149a's journal left as written): engine +0x104/+0x108 are the torque curve cursor's index and weight, not the throttle
  and a gear word.
- New test file crates/game-sim/tests/car_engine.rs.

## What the original does

The car is a body in the generic dynamics world. Its pre-force listener (`./kit fn 0x00398150`) calls the
shift once a frame; its force listener (`./kit fn 0x00397f40`) calls the engine step every substep, then the
four tire steps. The world runs `(int)(dt*115)+1` substeps of `dt/n`: four at 30 Hz.

**Engine step**, in order:

1. While the shift-throttle timer (+0xc4) is positive the box holds the clutch at 1 and uses its own throttle
   (up or down shift's, by +0xb0), and counts the timer down by dt; otherwise the car's throttle and clutch.
2. `curve_seek` the torque table at the revs, `curve_value` the torque (the bike's helpers, S8/move.rs).
3. NOX (+0x80 on, revs > 0, bottle > 0): `+ (5252*boost)/rpm`, the bottle `max(left - burn*dt, 0)`.
4. Net torque `torque * min(throttle + idle, 1) - rpm*(k*rpm)`.
5. Clutch up (≤ 0): the driven slots' spins summed (`(s0+s1)` or `(s0+s1)+(s2+s3)`), times 0.5 / 0.25, times
   the ratio, times 9.549296 → wheel rpm; slip = rpm − wheel rpm, or the revs if the wheel rpm is negative.
   Clutch in: wheel rpm = rpm, slip = 0, share = spin sum = 0.
6. Pump torque `slip*(slip*(scale*sign))`, held to ±max. Multiplication maxK at a stall, `maxK + (1-maxK)*(wr/rpm)`
   when the wheels turn slower than the engine. Output = pump * (boost * mult); boost = the car's launch boost
   (+0x6c) only with a positive ratio and spin sum.
7. Revs: `rpm += 9.549296*(dt*(r*(inv_inertia*(net - pump))))`, `r = 1/(1 + dt*(slope*inv_inertia))` with
   `slope = 0.5*(slip*(scale*(2*sign)))` (1 if that is ≤ 0); held to [MinRPM, redline].
8. Drive per slot `output * (tcs * (ratio*(share*(1-loss))))`, 0 while braking; added into the tire's +0x1f0.
   TCS (both tires of a pair on the ground) is a clamped lerp from 1 at MinTractionSlip to 0 at
   MaxTractionSlip of |slip ratio|.
9. The sound's pitch: revs' place in [min, redline] lerped onto [0.1, 1], + 0.0125 * curve-table sine of
   `35*clock` above 0.95, + 0.11 * smooth_noise(0.25*clock) below 0.2, − 0.1*(squat − 0.1) once the front pair's
   mean compression over slot 0's MaxCompression reaches 0.1; clock += dt; mapped back onto [min, redline].

**Shift** (once a frame): nothing when neither of slots 0/1 touches ground or in reverse; a running shift timer
(+0xac) counts down by dt and returns; with the clutch pedal up, the up and down points slide between their
low- and high-load RPMs by where the slip sits between LowLoadRPM and HighLoadRPM (clamped lerp). Over the
up point: next gear if there is one (shift-throttle timer = up duration, sound) — the timer restarts even in
top gear. Under the down point and gear ≥ 2: down a gear. Either restarts the timer at ShiftTime.

## How it was proved

- **Function oracle**, crates/game-sim/tests/car_engine.rs, on each S149a switched image (baja, buggy,
  truck): `the_engine_step_matches_the_original`, `the_shift_matches_the_original`,
  `the_gear_setters_match_the_original` — 12,000 generated cases per car each (the engine object's running
  words and constants around the file's, ties for the equal-ends branches, reverse, a four-wheel drive with the
  front pair in slots 2/3, TCS and NOX on and off, pedals, tire contacts/spins/slips/compressions). Every word
  the functions write plus the tires' +0x1f0/+0xa0 compared to the bit; the shift's sound call counted against
  the port's `Shifted`. Shifts covered: ≥1,089 up and ≥1,017 down per car. The pitch is in (with the real curve
  table out of RAM; smooth_noise hooked with a stand-in both sides read).
- **Captured drives**, `rpm_gear_and_drive_torque_replay_the_captured_drives`, S149a's baja, buggy, truck and
  truck-roll csvs, the engine from the disc through `Car::read_bytes`: 2,539 frames replayed, 0 words off —
  451 frames whole (all four substeps with the clutch in, from the first row), 2,088 frames' last substep (revs
  before it recovered from the slip, wheel rpm and engine braking it wrote; the spin from the wheel rpm), 35
  shifts. Every frame then runs the shift and compares rpm, gear, ratio, timers, cursor, converter and drive
  torques with the second row.
- Found on the way: a capture row is read after the frame's substeps **and** the next frame's shift (a shift's
  timer and shift-throttle timer land on the row unspent), and the pedals and launch boost the substeps read are
  the second row's (the car writes them before the world steps).

## Not verified / not 1:1

- The **sound's pitch** (+0x138) is not checked against the captures: it reads the suspension's compression
  mid-frame, after the previous substep's tire step, which no row keeps. It is proven in the oracle.
- The **smooth noise** under the pitch is hooked with a stand-in in the oracle; the port takes it as a closure.
  The real one is `noise::SmoothNoise` (proven in tests/noise.rs); its first-sample table build (rand draws) is
  the caller's business.
- **Coupled frames are only proven on their last substep** against the captures: the first three need the
  car's tire step (S149b) to know the spins. The oracle covers the coupled step in full.
- The last-substep replay may find more than one revs value that fits the row (adjacent floats with the same
  slip and braking); it accepts the frame if any one replays every word. It found a fit for every frame.
- **Rows that span two frames** (26 across the buggy and truck csvs — the sound clock ran 8 substeps) and
  paused rows (29) are skipped, counted in the test's output.
- NOX and TCS never fire in the captures (all four have +0x80 = 0 and TCS off); they are proven only in the
  oracle. No capture drives four-wheel or reverse either.
- The shift sound (`car_shift_sound`) is not ported — the port returns `Shifted` for the caller to play it.
- `Drivetrain::new` assumes a fresh engine's running words are zero (bar gear 1, launch boost 1, the bottle full,
  first's ratio); the constructor (`./kit fn 0x00399530`) was not compared word for word. A non-positive
  EngineInertia leaves the constructor's +0x60, taken as 0 here.
- Who calls `set_reverse` and `reset`, and with what, is the car's controls (S149b/S149d), not here.

## Next

S149d (the car step) hooks `Drivetrain` into the car's frame: the shift in the pre-force, the step before the
tires each substep, `DriveTire` mapped through `Engine::driven`, the pedals and launch boost written first. With
S149b's tire step it can then replay the coupled frames whole.
