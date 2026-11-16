# 1 — untamed bike damping (FINAL)

Task MD161c. 2026-11-16.

## What I did

- Checked the task's condition: "if the bike's body integrate is the atv_body one". **In the port it is not.**
  - The live bike ride on `Handling::Untamed` (and `MxVsAtv`) is `ride.rs`'s frame. Each substep ends in
    `ride.rs`'s `integrate`, which calls `chassis::integrate`: MX Unleashed's chassis, its own
    integrator. `ride.rs` never touches `atv_body`.
  - `chassis::integrate` has no angular damping lane at all (rates = advance + clamp + window). So there is
    no `d` to raise to `0.333/h`, and no switch to put it behind.
  - The only places a bike's `atv_body::Body` is integrated are SLUS-21104 harnesses:
    `tests/mxvatv_bike_ride.rs` (F31/F31b's frames against state 4) and `handling::ragdoll::step`
    (the rider's ragdoll world, called only from that test). Both must stay SLUS-21104's: they replay
    SLUS-21104's captures.
  - F31a's journal already set this out: the original's bike body is `atv_body::Body` (F31a,
    "Decision"), and the port's Ride carries MX Unleashed's chassis instead.
- So nothing was routed and no code changed. Every replay hash is unchanged because nothing moved.
- Filed **MD175** (`later`): call `Body::integrate_untamed` on Untamed once a Handling bike ride does
  integrate an `atv_body::Body` (F31b5's frame made a ride, or an Untamed bike frame harness), with this
  task's capture proof.
- Nothing taken from opentennis: it has no such body.

## What the original does

SLUS-21701's body integrate is shared by every machine's rigid body (research/untamed-machines.md, MD81c
table, "body integrate"). The bike's body is that body (F31a), so Untamed's bike does take
`powf(d, 0.333/h)` when a lane is under 1. MD161 ported it as `atv_body::Body::integrate_untamed`.

## How it was proved

Not proved against the original: there was no port code path to prove (above). The finding was checked by
reading the code: `ride.rs` (`integrate`, around the substep loop's `mv::Call::Integrate`),
`chassis.rs` (`integrate`), and a grep for `atv_body` and `.integrate(` over `crates/game-sim/src`.

## Not verified / not 1:1

- **Untamed's bike in the port has no per-time angular damping.** It also has no SLUS-21104 angular
  damping: the port's MX vs. ATV bike rides MX Unleashed's chassis integrator, not the original's
  `atv_body::Body` (F31a). Any ride where the original's bike body lowers a lane below 1 (contact
  collider damping, a wreck) differs. Reason: the port has no live Handling bike ride on `atv_body`;
  MD175 tracks it.
- **No Untamed bike capture with a damping lane under 1 was taken.** Reason: with no port path to compare
  against, a capture proves nothing yet; MD175 owns it.
- **Untamed's bike body was not checked to be the same vtable as the truck's.** Reason: taken from MD81c's
  static compare ("every machine's rigid body") and F31a's SLUS-21104 reading; no SLUS-21701 bike image
  was disassembled here.

## Next

MD175, when a Handling bike ride runs on `atv_body::Body`.
