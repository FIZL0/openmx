# 1 — atv engine note (FINAL)

Task MD162. 2026-11-16.

## What I did

The task asked whether MX vs. ATV pitches the quad's `vehicle.aud` loops off the engine rpm or off the
levers and speed, as MX Unleashed's bike does. **It is the levers and speed.** The quad's note is the
bike's engine-note class in the original too. Nothing reads the rpm. So the quad keeps the port's
`audio::Engine` (MD136). What changes is its inputs and one test in the decision. The quad also gets
its `[Wreck]` clank.

- `audio::AtvFrame { hands, crash }` (new). `AtvFrame::of(&quad::Ride)` reads the quad's words as the
  original wires them:
  - grounded: `q.on_ground` (any wheel down);
  - gas: `q.gas`, the gas lever after the input limiter's cut. MD136 used `q.gas_limited`, the
    rate-limited throttle out, which is what MX Unleashed's bike hands its note;
  - clutch: `bike.clutch`, the clutch lever as pressed. MD136 used `q.clutch_in`, the rate-limited
    clutch out;
  - speed: `q.speed_len`;
  - the throttle lock: `bike.held.lock.throttle`;
  - crash: the wreck word (`Ride::down`), the collision record's `touching`, and the length of
    `q.attitude` (the peak-rates vector).
- `Hands::throttle_locked(bool)` (new). The quad's decision revs freely on the ground when
  `0.1 < clutch || !throttle_enabled`; MX Unleashed's tests the clutch alone. The clutch hand is read
  nowhere else, so a locked throttle is passed as the clutch held in (clutch 1.0). I did this instead
  of adding a `Hands` field, because a new field breaks seven struct literals in other tasks' tests.
- `SeatFrame.atv` is now `Option<AtvFrame>`. `Sound::frame`'s quad branch feeds `hands` to the
  quad's note, as before, and ticks the seat's machine clank (`s.clank`, the plane's and heli's
  emitter) on the pack's `[Wreck]` list (else the bike's) with `crash`. A seat rides one machine at a
  time, so the quad shares that emitter. It is already stopped when a seat leaves and when the ride
  goes quiet.
- `main.rs` `atv_hands` now returns `audio::AtvFrame::of` of the ridden quad.
- Wreck rule kept: while down, the hands are off the levers (`Hands::wrecked`). That is the user's
  earlier rule for the bike (2026-10-30), not the original's.

## What the original does

SLUS-21104 (atv program). The rows are in research/oracle.txt under MD162.

- `./kit fn 0x002d07b0` (VehicleSound) builds the engine note and the wreck emitter for a vehicle.
  On PC, VehicleSound (0x00606730/0x00603930) builds MotocrossEngineSoundEmitter only for vehicle
  types 0 (motocross) and 1 (atv). So it is one class for both.
- `./kit fn 0x004a2d40` is the engine note's constructor. Its layout is MX Unleashed's: lists at
  +0x204..+0x2ac, state at +0x2d8.
- `./kit fn 0x004a3520` is its decision. It matches the port's `Brain::decide` line for line except
  the grounded branch's first test: `0.1 < clutch || throttle_enabled == 0`.
- `./kit fn 0x004bb710` and `./kit fn 0x004dacf0` wire the five hands (emitter +0x200), relative to
  the vehicle. On PC they are MXVehicleMotocross::vf35 / MXVehicleAtv::vf35.
  - +0x324: on the ground
  - +0xdd4: throttle enabled
  - +0x314: the gas lever the limiter leaves, after its cut (not +0x318, the rate-limited out)
  - +0x32c: speed
  - +0x12c0: the clutch lever in (not +0x12d4, the rate-limited out)

  MX Unleashed's wiring hands its note the rate-limited gas instead.
- With the throttle locked, the port's input limiter (`handling::step::input_limiter`) leaves the gas
  lever word as pressed and holds the throttle out at 0. So a locked quad revs on the gas through
  `Rev`/`Panic`, as at a start gate.
- `[Wreck]`: the loader is `./kit fn 0x004ffe20`, the machine wreck loader. It reads
  MinImpulse/MaxImpulse with no ×1.4667 scaling.
  - `./kit fn 0x00500000` wires it to the body position and velocity, the wreck word quad+0xc80,
    and the collision record's touching flag.
  - `./kit fn 0x00500350` points its impulse at quad+0x1250, the peak-rates vector.
  - `./kit fn 0x005001e0` is its frame: gate and touching and idle lead to `./kit fn 0x00500040`,
    a no-repeat pick at clamp(|impulse|/Max×100), with the impulse at least Min.
  - This is exactly the port's `WreckClank::tick` on a `PlaneCrash`.

## How it was proved

`crates/openmx/tests/atv_engine_note.rs`, through `./kit check -- -p openmx --test atv_engine_note`
(passes):

- `a_locked_throttle_revs_the_quads_note`: the shell's sound on made-up quad frames. Gas from idle on
  the ground at a standstill goes to `LowAccel` unlocked and to `Rev` locked, and stays `Rev`/`Panic`.
  `throttle_locked` changes the clutch alone.
- `the_quad_clanks_on_its_wreck`: a down, touching, hard enough frame plays the pack's clank at 0.5
  (impulse 10 of Max 20). Not down, not touching, or under MinImpulse plays none.
- `the_quads_note_follows_its_levers`: rides the pack's Race 250 P on a flat floor, 90 ticks on the
  gas. Every tick the hands are the gas lever, the clutch lever, the ground flag, the speed and the
  position. At least one tick has the lever at 1 while the rate-limited throttle is still under 1.
  A clone with the throttle lock set hands the note clutch 1.

The decision's match was read off the disassembly against `Brain::decide`. MX Unleashed's oracle test
(`tests/audio.rs` `the_engine_note_matches_the_original`) covers the shared part. The quad's one
different test was not run through an oracle.

## Not verified / not 1:1

- **Not heard.** Headless runs have no audio device. Not listened to beside the original.
- **No oracle or capture of the quad's decision.** The `|| !throttle_enabled` test and the hands'
  offsets were read off the disassembly. The decision was not run in the Ee interpreter against the
  port, and there was no PCSX2 capture of the quad's note.
- **The throttle lock never sets on the port's quad.** Nothing in the port's quad sets
  `bike.held.lock.throttle` yet (there are no start gates for it). The path is tested on a clone
  only.
- **The clutch hand is always 0 in a ride.** `quad::Ride::controls` never copies the pad's clutch into
  `bike.clutch`. So the hand is right (it is the original's word) but empty. Filed as MD175.
- **The clank's impulse while down.** Whether the original's PreProcess still resets and refills the
  peak-rates vector (+0x1250) while the quad is wrecked, and so whether the port's `q.attitude` holds
  the same value then, was not checked. Nor was the wrecked quad's `touching` flag.
- **The hands go off the levers while wrecked.** That is the port's rule (user, 2026-10-30), not the
  original's. The original's note reads the levers as they are.
- **The shared clank emitter.** The original gives the quad a wreck emitter of its own. The port
  reuses the seat's machine clank, which only matters if a seat could be on two machines at once.

## Next

Done. MD175 gives the quad the pad's clutch lever.
