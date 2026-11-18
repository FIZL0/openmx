# 1 — mxvatv 360 r1 hold (FINAL)

Task S478. 2026-11-18.

## What I did

- `air::Spin::held` (the port's own): R1 alone on the 360's last step (`Step::modifier`), set beside
  `three_sixty` in `ride.rs`. On the wire, not hashed (animation only, as `starts`). Not set on
  Untamed's own 360 (see below).
- `vehicle_anim::Request::spin_hold`, filled by the rider's body request (`rider.rs`) and the bike's
  `Chassis::request` (`game-render` `vehicle.rs`) from it. Skipped on the wire, as `spin_starts`.
- `vehicle_anim::Driver` gets `hold` and `shake`: the original's hold clock and shake clock. On the wire.
- The 360 arm of `Driver::frame`:
  - R1 alone held and the clock in [0.72, 0.72 + 0.04]: the clock stops. The rider's phase is
    (clock + shake) / 1.5; the bike's is the frozen clock over its clip. The hold and shake clocks run on.
  - Otherwise as before (clock += dt), but the bike plays phase 0 on the tick its 360 (re)starts.
- `Driver::spin_shake`: the trick hold's ramp and shake, as `handling::update::trick_hold` has it.
- Re-pinned `size_of::<Sim>()` (12360) and the riders' wire bytes (13648) for the two driver floats.
- Nothing from ../opentennis: it has no stunts.
- No live run of the original; nothing in PCSX2.

## What the original does

Recipes: trick step `./kit dis 0x004e49c0 0x004e4c80 -p atv`; trick hold `./kit dis 0x004e4810 0x004e4900 -p atv`;
bike animate `./kit dis 0x004dd8f0 0x004ddc80 -p atv`.

- The trick step zeroes the held word every frame. A turning 360 is a kept, driven stunt: kind 4. With
  the asked low nibble 4 (R1 alone) and the clock in [min(0.72, 0.3), 0.72 + 0.04] it posts the stunt
  mark; with the clock at or past 0.72 too it calls the hold and sets the held word, and does not add dt.
  So the clock stops on the first tick it is at or past 0.72 (tick 22, 0.7333), and the hold runs from
  tick 23 for as long as R1 alone stays held. Let go, the clock runs on, and it is past 0.76 at once,
  so a second press never holds again on that start.
- The hold: ramp = clamp(lerp 0..0.07 over hold clock 0..0.5 s); shake = ramp × noise(2 × shake clock);
  the shake, hold and third clocks += dt; phase = (clock + shake) / 1.5.
- Writers (whole-overlay scan): the shake clock `+0x14e0` is written only by the constructor and the
  hold, so it never resets in a ride. The hold clock `+0x14dc` is zeroed by the stunt start and the
  remote stunt clock.
- The rider plays the stored phase (with the shake). The bike animate plays its clip at the stunt
  clock over the clip length (no shake, frozen while held). On a new stunt id or the restart ask, it plays
  phase 0 that tick (skips the divide).

## How it was proved

Function oracle on state04's RAM image, the original's own code, tick by tick
(`crates/game-sim/tests/mxvatv_stunt_cross.rs`):

- `the_ports_rider_holds_a_360_under_r1_as_the_original`: R1 alone 60 ticks, let go 20, held again 10.
  The trick step, rider layer and bike animate run each tick. The port's rider fade, phase, stunt clock
  and both hold clocks match bit for bit on all 90 ticks. Held exactly ticks 23–60 (38 ticks). The shake
  moves the phase on more than 30 of them. The smooth noise is hooked on the original's side to the
  port's noise, so the shake's arithmetic and clocks are proved, not the waveform.
- `the_ports_bike_holds_a_360_under_r1_as_the_original`: the same ride. The phase the original's bike
  animate hands its one-clip play (hooked to record `f12`) matches the port's on every tick.
- The MD248 and S467 bike tests now compare against that recorded play phase too, not a computed
  clock / length. Phase 0 on each (re)start tick is asserted.
- `r1_alone_before_the_clocks_stop_does_not_hold`: port-only edge check.
- Mutant check: zeroing the shake and dropping the restart-tick 0 failed four tests; restored.

`./kit check -- -p game-sim --test mxvatv_stunt_cross`: 12 passed (the image is present; the mutant run
shows they do not skip).

## Not verified / not 1:1

- **The shake's waveform.** The port shakes by `ride::noise`, the stand-in the MX Unleashed rider's
  balance uses. The original's smooth noise reads tables built from its session's `rand()`; this ride
  has no such tables. The test hooks the original to the same stand-in, so only the waveform differs.
- **The stunt mark** (`0x0046b110`, posted with R1 alone while the clock is in [0.3, 0.76]) is not
  ported here. It is a score/notice call, not the animation. Not checked whether the port's MX vs. ATV
  card needs it.
- **Untamed's 360** does not set `Spin::held`, so it never holds. Its trick step was not read; it may
  or may not hold the same way.
- **Not watched live.** No 360 ridden with R1 in PCSX2 or the shell. The tests poke the flag.
- **The tick R1 is read on.** The port's flag is this tick's pad. The original reads the pad reader's
  byte for this frame. Assumed the same tick, as `Step::modifier` already is; not run side by side.
- **Kind 0 holds.** The original also holds a 360 at 0.72 when no trick button is held and its torque
  is off (kind 0 = asked 0). In the port a shown 360 always has its torque applied, so this case does
  not come up. Not proved that it never can.
- **Wire growth.** Two floats per rider on the snapshot (and a byte for a trick rider). Its effect on
  the netplay tests is whatever the gate shows.
- **Only state04 and the right 360.**

## Next

None.
