# 1 — the limiter and the steer ramp (FINAL)

## What the original does

**atv_bike_input_limiter** (bike vtable +0x34c) is MX Unleashed's limiter with the fields moved (map in
`research/oracle.txt` `# MD63a`) and four differences:

- It does not square the stick. The stick x/y go into the axis walk as they are.
- It calls 0x004a7990 first, which zeroes the gas when the object at bike +0x680 has +0x1d0 > 0. That is
  MX Unleashed's start-gate penalty card (+0x1c0 there), so the port reads it as `Controls::gas_cut`.
- A gas cut: while the throttle lever is off (+0xdd4 = 0) the latch +0x1764 is set and the timer +0x1768
  zeroed. Once the throttle is back, the gas is held at 0 and the timer climbs by dt until the gas is
  let go or the timer passes +0x176c. +0x176c is only ever zeroed (bike reset), so the cut lasts two
  frames: the frame the throttle comes on (timer 0, not past 0) and the next (timer dt > 0 lets go after
  zeroing that frame's gas too — the test `mxvatv_gas_cut_holds_two_frames` pins it).
- The gas gate reads "the rider's stunt id equals the balance stunt" (+0x1524 == +0x152c) where MX
  Unleashed reads hands-off. The port feeds it `Controls::hands_off`.

**atv_bike_ground_head**'s steer ramp: when +0x12f0 is set, a timer +0x12fc runs while |lean| ≥ 0.987
(zeroed under it), and the command is `lean·amount·min(1, t/time) + lean·fade·(lock − amount)`. Off, it is
`lock·(lean·fade)`. amount is InputRampAmount in degrees × 0.01745329. This is the same arithmetic as
quad.rs's proved full-lock branch.

## What I did

- `handling/step.rs`: `GasCut`, `GAS_CUT_TIME` (0), `LimiterIn`, `input_limiter`; `SteerRamp`
  (`of_file`), `HARD_OVER`, `steer_command`.
- `phy.rs`: `Phy::steer_ramp` from [Joystick]; `on` stays false — +0x12f0 is 0 in both RAM images and no
  bike-relative store to it was found in the ELF.
- `ride.rs`: step 1 runs `input_limiter` on an MX vs. ATV ride (instead of `Held::step`); after the
  front's `mv::pose`, a not-balancing MX vs. ATV ride takes `steer_command`. `limit`, `toward`, `rise`
  became `pub(crate)` for the limiter.
- `handling::Flip` carries `gas_cut`, `gas_cut_timer`, `steer_ramp_timer` (hashed, on the wire).

## How it was proved

- `mxvatv_input_limiter_is_the_originals`: 10,000 random states and inputs, the whole limiter run in the
  Ee with 0x004a7990 left in (its object poked at a scratch address), every output field compared word
  for word; each gas path (throttle off, cut let go, cut timed out, cut holding, balance stunt, reverse
  on the ground, gate cut, gas walks, start latch, no latch) taken ≥ 100 times.
- `mxvatv_steer_ramp_is_the_originals`: 10,000 states through the whole ground head with the limiter,
  head tail, auto shift, clutch and body vt +0x64 hooked out; +0x12c8, the front wheel's +0x8ac and the
  timer compared. Paths: no ramp, let go, coming in, full, flat fade.
- Mutations that each red the right oracle: squaring the stick, not zeroing the gas in the cut, dropping
  airborne or hands-off from the gas gate, `<=` in the cut timer compare, `<=` at 0.987, re-associating
  `lean·(amount·t)` and `lock·(lean·fade)`.
- `mxvatv_ride.rs` `the_steer_ramp_and_the_unsquared_stick_reach_an_mxvatv_bike_only`: on a ride the
  ATV stick is unsquared and the ramp changes the command; on Original neither shows.
- `./kit check -- -p game-sim --test mxvatv_step --test mxvatv_ride --test mxvatv_air --test hash
  --test levers --test snapshot` and `cargo build -p openmx`.

opentennis: nothing to take (no shared input code).

## Not verified / not 1:1

- Who sets +0x12f0 is unknown; the ramp is off on every bike unless a caller sets `steer_ramp.on`.
- The +0x680 object is taken to be the start-gate penalty by analogy with MX Unleashed; in state04 its
  +0x1d0 is 0 and nothing in this task drove it.
- +0x152c is taken to be the balance (hands-off) stunt; not traced through the stunt table.
- A lever rate of 0 stays the port's "no limit"; the original would hold the lever still. The oracle
  only draws rates > 0.
- The rest of the ATV ground head (its control-off and balancing branches) is not ported; a balancing
  ride keeps the port's own command.
- No MX vs. ATV ride capture exists, so no whole ride is replayed bit-exact. The preload under
  MX vs. ATV now sees the unsquared stick, which is what the original reads, but not compared on a ride.
