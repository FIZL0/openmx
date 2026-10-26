# 1 — one player start drop in (FINAL)

Task P171. 2026-10-26.

## What I did

- `join::FEWEST` 2 → 1: Start on the join screen is done with the host alone. A local game of one
  skips the split-layout screen (there is nothing to lay out) and goes straight to Tracks; `--screen
  split` still seats a second pad so it has something to show.
- Why Free Ride "did not take" local players: I drove the real release shell with two stand-in
  fifo pads (new `OPENMX_PAD_ROOT` env in `Seats::open`, laid out like pad.rs's test fakes) from the
  splash to Free Ride by pad presses alone. Solo on pad A, pad B's Cross joined ("player 2 joined on
  pad seat 2; views: 2 players, stacked"); solo on pad B, pad A joined. So `seats_open`,
  `ride_devices` and the Cross path all work. What a player saw was nothing telling them so: the
  ride had no cue at all, and the pause menu's hint drew "? on a free controller joins" because the
  menu font has no ✕ glyph. Fixes: the pause hint now says "Cross"; the ride HUD shows a "Press ⓧ
  to join" pill in the top-right column (under the markers panel / camera toast) of the top-right
  view while `Menu::join_cue` holds — seats open, fewer than `join::IN_RIDE` riders, and some
  pad seat that is not riding pressed nothing (an idle seated pad). Gone once seated or full. The
  Original overlay draws the same pill with sprites (`ride_hud::join_sprites` / `join_draw`).
- Second cause found, filed as P173: when 1P is a pad the keyboard has no pad seat, so the keyboard
  cannot drop in on a pad-1P ride. With keyboard 1P, pads take seats 1+ and join fine
  (pad.rs `the_seats_start_from_1p`, `devices_report_on_their_own_and_1p_takes_the_first_seat`).
- Tests: tests/join.rs `start_needs_only_the_host`; tests/menu.rs
  `one_pad_starts_a_local_game_alone_and_can_free_ride` and
  `a_lone_rider_on_any_pad_seat_is_joined_by_another_and_the_cue_says_how` (Free Ride solo on
  seat 0 → seat 1's Cross joins, split goes Stacked, cue on while free and off at four / with seats
  closed; a local game of one on pad seat 1 (keyboard 1P case) joined by pad seat 0).

Nothing taken from opentennis beyond the join screen that was already modelled on it.

## What the original does

Not applicable: the original has no drop-in; this is a shell feature.

## How it was proved

Menu-level tests above (pure state); a live run of the release shell with two stand-in pads
(scratch script, two runs, both joined); the cue looked at by eye in a `--screen pause --shot` frame.

## Not verified / not 1:1

- Not checked with two real pads: none are plugged in here, and the shell skips uinput fakes; the
  live run used fifo stand-ins through `OPENMX_PAD_ROOT`.
- Keyboard-solo Free Ride joined by a pad was not run live (no keystrokes reach the window here);
  covered by the PadSlots tests and the menu test on pad seat 1 only.
- The cue was seen by eye only on the HUD overlay in one shot; the Original-overlay sprite cue and
  the CPU-renderer path were not looked at.
- Cue placement in a 3- or 4-way split (it goes to the top-right view) was not looked at by eye.
- The keyboard cannot join a pad-1P ride (P173).

## Next

Lands as is.
