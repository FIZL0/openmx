# 1 — body wreck lag (FINAL)

Task S91. 2026-10-26.

## What I did

- `Rider::body_frame` now asks the body's mode machine about the wreck with `PutBack::wreck` alone
  (it was `state.wreck.is_some() || put_back.wreck`). The fill sits after the physics and before the
  machine's verdict, so on a wreck's entry tick the body is still riding and enters `Mode::Wrecked`
  the tick after; the put-back side (still down on the clock's tick, S74) is unchanged because it was
  already that same field. The `state` binding in `body_frame` went with it; the request table in its
  doc says where the wreck word comes from now.
- `tests/rider_anim.rs`:
  - `the_put_back_holds_the_body_as_the_original_does` asserts the entry instead of counting it: on
    the entry tick neither the original's request/mode nor the port's `PutBack::wreck` has the wreck,
    on the tick after both do (`asked_wreck`, mode 2), and every entry in context/s74 must be proved
    (≥3, all of them).
  - `a_wrecked_body_settles_and_then_holds_still` drives a whole `Rider` and asserts its body is not
    in the wreck on the verdict's tick and is the tick after (the flail asserts skip the entry tick).

  - `the_rider_publishes_the_body_its_own_request` was riding the gate under the wreck mode: its
    rider is dropped onto the floor and wrecks (reason 8) on tick 0, so before this the body went
    straight into `Mode::Wrecked` and never ran the gate mode's entry. With the lag tick 0 is
    `Mode::Gate`, whose entry draws a unit and starts a breathe of up to 0.5 s, and the sequence grew
    a leading `GET_READY` ([30, 46, 54, 46, 59]). The test now turns that rider's wrecks off (it is
    about the gate, nothing asking), asserts the body is in `Mode::Gate` every frame, and expects the
    five states, the entry's breathe first.

Nothing taken from opentennis: it has no body/ragdoll request.

## What the original does

S74's journal measured it: the bike's update fills the body's request (wreck reason copied, placed
word copied) after its physics and before the ride machine's verdict, and the body's update reads
the request later in the tick. So a reason set by this tick's verdict reaches the body on the next
fill. context/s74 shows it on all three entries: the entry tick has request reason 0 and body mode 1,
the next tick request wreck and mode 2.

## How it was proved

- A PINE capture compared tick by tick: context/s74's five files (copied into this slot from s3),
  2695 ticks, 7 chord and 2 clock put-backs, **3 wreck entries, 3 proved** (checked by raising the
  assert to 4 once: "only 3 of 3 wreck entries proved"). `PutBack::wreck` equals the original's
  request wreck on every one of the 2695 ticks (S74's assert, unchanged), and the body is now asked
  with exactly that field.
- The shared generator: a ragdoll rider's wreck entry draws nothing in `Body::enter_wreck`
  (`Down::enter_from` senses at once, so `Thrown::launch` is always `Some` by the time the body
  enters, entry tick or the tick after). Only a rider with wreck files and no ragdoll draws the five
  numbers there, now a tick later — after the next tick's ride-side draws (an off-map rider's
  re-throw), which is the original's order, the body updating after the bike. The replay hashes are
  the gate's: `./kit check --all` green in `./kit done`.

## Not verified / not 1:1

- The late draw order for a wreck-files-without-ragdoll rider (five `enter_wreck` draws now after
  the next tick's ride draws) is argued from the update order, not captured: no capture has the
  shared generator's state around a wreck entry, and the shipped game rides with a ragdoll, where
  the entry draws nothing.
- The full rider's settle timing (`a_wrecked_body_settles_and_then_holds_still`) is the port's own
  stand-in sensors on a flat field, not the original; the one-tick-later entry did not move its
  `ROLL_TICKS` assert, but that test is not a comparison with the original.

- The gate entry's leading breathe in `the_rider_publishes_the_body_its_own_request` is the ported
  mode machine's own entry (V19), now reached; no capture of the original's body on a rider's first
  tick at a gate backs that particular sequence.

## Next

-
