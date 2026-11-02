# 1 — rider lean (FINAL)

Task S228 (P227's ask). 2026-11-02.

## What I did

I added a request-axes check to `p1_rides_each_event_as_the_original`
(crates/game-sim/tests/fly_and_collide.rs). It compares the port's `rider.packed_pose` and
`packed_fore_aft`, latched in step, with the capture's p1_req_axes x/y. Each row is checked against
the port's tick t or one of the two ticks after it, to allow for the row-phase drift.

## What the original does

The body's motion request carries the stick's lean (x) and fore-aft (y). The rider's step latches
them, and the drawn pose eases toward them. After a wreck, the body reads the wreck's request
instead.

## How it was proved

The capture (context/capture/s228/fly.csv) against a bit-exact port sim replay, run with `./kit check
-- -p game-sim --test fly_and_collide`; it passes. The axes match on every riding tick of the nine
events, apart from three cases: the first tick (the motion driver is not seeded from the image),
ticks the capture skipped, and ticks after a wreck.

## Not verified / not 1:1

- **The drawn rider is not compared** (crates/game-render/src/rider.rs, crates/openmx/src/main.rs).
  The original's drawn joint pose through turns was not captured on this tape, so whether the port's
  drawn lean matches is unproven. That is S228a. If the sim's request is exact and the drawn lean
  still looks small, the parting lies in the easing or skinning on the render side.

## Next

S228a.
