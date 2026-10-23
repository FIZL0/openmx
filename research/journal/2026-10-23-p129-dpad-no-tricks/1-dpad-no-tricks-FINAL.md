# 1 — dpad no tricks (FINAL)

Task P129. 2026-10-23.

## What I did

- **P93's exception is gone.** `Controls::stick_alone` (crates/game-sim/src/ride.rs) now always lets
  the d-pad go — `dpad_y`, `dpad_dir` and `dpad` cleared — whatever trick or extra button is held. The
  left stick alone is a trick's direction in the shell, which the original reads too
  (`trick::stick`: the analogue deflection, overridden by the d-pad only where it is pushed).
- **A held trick button no longer stops a marker.** It did: `markers::fired`, `held` and `holding`
  each returned nothing with a trick button down. All three lost the test and their `hands`
  parameter (`hud::MarkerBar::read` with them); `fly`'s private `pressed` is now `fired` itself.
  `markers::frame` / `frame_among` keep their `hands` argument (unused now, `_hands`) so other
  branches' calls still compile.
- **Callers checked.** `Seat::pass` (main.rs), `Rides::step` (net/rides.rs) and the tests' own
  passes all ride on `stick_alone`; the biplane's `without_dpad` already cleared the d-pad.
  `Controls::through` is untouched, so every replay and sim test rides on the original's d-pad.
- **A Markers switch per seat (the user's 10-23 addition).** `Settings::markers: [bool; 8]`, on by
  default, written to settings.txt only when one is off (`markers = on off on`, the last word for
  every seat past it, like `camera`'s line); a word it cannot read keeps them on. A Markers row on the
  Controls tab, for the Player row's seat. `--markers on|off[,…]` on the command line, over the
  settings. `Departure::Markers` under P126's Original switch: On/Off, off is the original's, so
  Original on turns every seat's off and the switch reads Mixed with one seat's on.
- **What the switch does.** `vehicle::hands(polled, markers)`: on, `stick_alone`; off, the poll as it is
  (`Controls::through`). Off, `Garage::pass_marked(.., false)` runs no bike or plane markers, the
  plane reads the d-pad (`Plane::dpad`, set by `Garage::set_dpad` each pass), and the seat's marker
  bar is not drawn (it is left out of `shown_markers`, which both of P117's draw sites read).
  The seat's switch is read live from the menu each frame, so a change under the pause rides from the
  frame it is made; a tape plays under `--markers` (on unless given), as it does under
  `--biplane-controls`.
- **Online.** The switch is a seat's choice every machine must step the same, so it rides in the
  seat's `SWITCH` byte as bit 4 (`Wanted::dpad_rides`); 0, an idle or guessed pad's, is markers on.
  `Rides::step` reads it for the hands, the bike's markers and the plane's (`fly_lane` sets
  `Plane::dpad` from it).
- **New layout with markers off: the d-pad rides as the original's too.** Decision, since nobody can
  be asked: New only moves the levers (R2/L2), and its binding table has the d-pad on the same four
  commands as the original's; with the markers off nothing else wants the d-pad, so leaving it dead
  would be four dead buttons, and one rule ("markers off ⇒ `Controls::through`") is simpler to
  explain on the row than one per layout; the row's hint names no layout.
- Nothing taken from opentennis: it has no d-pad markers or trick gestures.

## What the original does

The original's d-pad is commands 0–3 of its binding table (P73): a digital stick (`trick::stick`
forces an axis to ±1) and the direction half of a trick's gesture, with or without a trick button.
With markers off the shell hands the bike exactly that (`Controls::through`); with markers on the
shell's hands have no d-pad at all, and the left stick is the gesture's direction as the original's
analogue stick is.

## How it was proved

- `crates/game-sim/tests/ride.rs` `stick_alone_does_no_trick_off_the_dpad`: on the disc's 125 off
  the flip ramp (needs the disc; proved it ran with a temporary panic printing the starts), Circle with
  d-pad Up in the air starts a trick on the original's hands (Trick(0)), starts nothing through
  `stick_alone` and rides bit-identical to Circle alone, and the stick pushed up starts the same
  tricks as the d-pad did.
- `crates/openmx/tests/markers.rs` `a_trick_button_does_not_take_the_dpad_from_the_markers`: Down held
  with Circle places at `HOLD_MS`, Left with Circle steps, the bike rides as on Circle and gas alone.
  `markers_off_the_dpad_rides_as_the_originals`: markers off, d-pad Left and Down+Circle ride tick for
  tick as `Sim::poll` + `Sim::step` (the original's hands) and fire nothing; markers on, the same pads
  ride as with no d-pad and fire Stepped / Placed. `the_bar_fills_with_an_up_or_down_hold` and
  `flying_the_plane_shows_the_planes_bar` now expect the bar to fill with a trick button down.
  `a_trick_gesture_is_not_a_marker` (P93's pin) is removed.
- `crates/openmx/src/same_ride.rs` `markers_off_online_is_the_offline_ride`: a seat with the bit set
  online is the offline seat with markers off, state for state, over the wild pads that press the
  d-pad; and differs from the markers-on ride.
- `crates/openmx/tests/menu.rs` `each_seat_has_a_markers_row_saved_in_the_settings` (row, per seat,
  file round trip, Original on/off), and P126's own tests cover `Departure::Markers` through
  `Departure::ALL`. netplay.rs: the Wanted byte round trip covers the new bit; the mashed-pads test
  now asserts the chord's d-pad never reaches the hands.
- The replay tests ride on `Controls::through`, which did not change; the full gate passes them.

## Not verified / not 1:1

- The shell's Seat::pass itself (main.rs, the binary) is not run by a test: its choice is
  `vehicle::hands` and `Garage::pass_marked`, which are, and `same_ride.rs` runs the offline seat with
  the switch both ways.
- Under the New layout with markers off the d-pad's trick gesture and steering were proved on the
  shipped (Original) table only; New's table has the d-pad on the same commands, but no test rides a
  New-scheme report with markers off.
- The plane with markers off flying on the d-pad (`Plane::dpad`) is not tested: the d-pad's meaning
  in the plane is `flight::pad`'s, unchanged since before P107, and only the `without_dpad` call is
  skipped.
- A tape records no Markers switch: one made with markers off plays back right only under
  `--markers off`. Not checked by a test.
- Not run in the windowed shell or against PCSX2: nothing here touches the simulation, only which
  hands reach it, and every proof above is the port against itself and the original's input layer.

## Next

Nothing.
