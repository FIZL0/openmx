# 1 — first controller takes 1p from touch (FINAL)

Task P321. 2026-11-06.

## What I did

User, 2026-11-05: on mobile the touch controls and a 2P should never be up together; the first
controller always takes over 1P, mid-ride too. P212 (f302ce4b) had a controller connecting mid-ride
join as 2P beside a kept touch pad.

- `ios.rs`, `android.rs`: the touch pad is listed only while no controller is
  (`pad::touch_listed`, one rule for both). The P212 "kept mid-ride" state (`TouchSeat`, `SEAT`,
  `touch_seat`) is gone, so a controller connecting mid-ride takes the touch pad off the list in the
  same poll and `PadSlots::touch_swap` hands it the touch player's seats and 1P's device, exactly as
  in the menus. No split; the touch controls hide (the overlay follows `TouchPad::shown`). The last
  controller going lists the touch pad again and it takes back the seats they were waiting in
  (`touch_swap`'s existing hand-back). A second controller is new and not in use, so `update` seats
  it as 2P as before.
- `pad.rs`: `Seats::poll` lost its `riding` flag (its only reader was `touch_seat`), and
  `PadSlots::wanted` went with it (only `touch_seat` used it). The web's touch pad (web/touch.js) now
  goes through the same `touch_swap` (`Seats::touch_hand_over` for wasm32, finding the touch id by
  `is_touch` among the listed pads or the seats, via the new `PadSlots::in_use_by`): touch.js already
  hides itself the frame a real pad connects, which used to leave 1P paused waiting for the touch pad
  while the pad joined as 2P.
- `main.rs` (`android_so.rs` is a symlink to it): the menus' "1P follows a hand-over" now runs on
  Android and the web too (the web only when one side is the touch pad), so the menus keep 1P rather
  than bring the splash back.
- Tests (`pad.rs`): new `a_controller_mid_ride_takes_1p_from_the_touch_pad` — touch pad 1P riding,
  a controller connects: the phone's list (built with `touch_listed`) has no touch pad, the
  controller holds seat 0 and 1P with no seat 1 and nobody waiting; a second controller joins 2P;
  both go and the touch pad is 1P again. `a_new_controller_mid_ride_still_joins_beside_the_touch_pad`
  (P297's statement of P212's rule) is replaced by it; the P297 reconnect test drops its "touch pad
  still listed beside the controllers" step, which cannot happen now; `a_touch_pad_taken_off_every_seat_is_not_wanted`
  went with `wanted`.

opentennis's seats.rs (`hold_touch`) solves a different thing (whether the touch pad holds a seat);
nothing taken.

The rest of the user's message (remove Resume Ride, kick a player whose controller is removed on
the Controllers screen, mph top left under fps) is other tasks, not P321's.

## What the original does

Nothing: front-end and pad handling are ours (AGENT.md: menus and front end are ours to design).

## How it was proved

Unit tests in `crates/openmx/src/pad.rs` (host build), `./kit check` (14 touch tests pass). The
other targets type-check with no new warnings: the wasm32 lib (web.sh's cached cross toolchain),
the Android example (`--features android,ship`, rustup 1.99.1 + NDK clang, `target/android`), and
the iOS bin (`tools/ios-build.sh check`).

## Not verified / not 1:1

- Not run on an iPhone or an Android phone, nor in a phone browser with a real pad: no device or pad
  here (uinput is root-only). The seat logic is tested on the host with the phone's listing rule;
  ios.rs/android.rs are only type-checked for their targets, not run, and the gate builds neither.
- The web path's touch id lookup (`is_touch` over the listed pads and seats) is exercised only by
  type-checking, not by a browser test with a pad connecting mid-ride.
- Any "gone" pad's seat goes to the touch pad when it is listed again (touch_swap's hand-back, P212's
  rule, unchanged): if 1P is the keyboard and a 2P's pad drops on a touch screen, the touch pad would
  take 2P. Not changed here; P321 is about 1P.

## Next

Nothing for P321.
