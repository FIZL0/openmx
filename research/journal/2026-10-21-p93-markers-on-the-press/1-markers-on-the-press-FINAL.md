# 1 — markers on the press (FINAL)

Task P93. 2026-10-21.

## What I did

- **All four marker directions fire on the press** (`markers::fired` reads `Table::pressed`, the
  command's press edge, on either binding). `markers::HOLD_MS` and `markers::holding` are gone; a
  direction held fires once, on the tick it went down.
- **The decision on Up/Down:** they go on the press too, with no hold kept against an accidental
  teleport. Why: once the d-pad no longer steers (below), nothing else in a ride asks for it without
  a trick button, so a press with no trick button is meant for the markers. The user's own words were
  "fire on the d-pad at once". The one way left to press one by accident is starting a trick gesture
  d-pad first (the direction a tick or more before the trick button): Left/Right then step the
  selection (harmless), Down re-places the selected slot, Up teleports. A gesture with the button down
  first, or on the stick, fires nothing, as before.
- **The d-pad out of the ride in the shell:** `Controls::stick_alone` (ride.rs) clears `dpad`,
  `dpad_y` and `dpad_dir` when no trick button is held and leaves everything as it is when one is,
  so the gesture's d-pad (direction, and the digital stick the air steps read with it) is the
  original's. The shell's two ride paths call it after the sim's poll: `Seat::pass` (main.rs) and
  online `net::rides::Rides::step`. `Controls::through` / `Sim::poll` are untouched: they are the
  original's reading, which every replay fixture and capture comparison rides on. It applies under
  both of the shell's layouts (New and Original), since the markers are on the d-pad in both.
- **HUD:** the slot display's hold bar is gone (no hold left to fill) and the panel is one bar's
  height shorter. The banner stays up while a marker direction is held (`markers::held`), so a press
  that changes nothing — Up on an empty slot — still shows what it was on.
- PLAN.md's line for markers.rs says press, not 250 ms.
- opentennis: nothing to take; it has no d-pad markers.

## What the original does

Nothing of the markers: they are the port's own. The original's d-pad in a ride is a digital stick
plus the gesture's direction (P73's commands 0–3); the shell now departs from it on purpose for the
digital-stick half, by the user's call (2026-10-21).

## How it was proved

`crates/openmx/tests/markers.rs`, ridden as the shell rides (poll → `stick_alone` → step → markers):

- `a_one_tick_press_picks_the_next_slot`: a one-tick Right picks slot 2 on that tick, Left steps
  back, Left+Right together does nothing.
- `down_places_a_marker_on_the_press`: placed on tick 0 of a 40-tick hold, once; a second press places
  again; a 1 s Right steps once; Up on an empty slot is refused on the press.
- `a_held_dpad_moves_no_bike`: each of the four held 60 ticks on the gas rides bit for bit as the gas
  alone (ride hash every tick); control: the original's reading of the same pad (`Sim::poll`) ends
  somewhere else for each direction.
- `a_trick_gesture_is_not_a_marker`: Down with Circle down fires nothing and rides exactly as the
  unmasked original reading; control: Circle alone rides differently, so the d-pad was read.
- The P88 tests (teleport, run-up repeat, player picks, snapshot) moved from 250 ms holds to presses.

## Not verified / not 1:1

- The shell's d-pad no longer leans or steers the bike with no trick button down: deliberately not
  1:1 with the original (user's request). Replays and fixtures still ride the original's reading.
- A d-pad-first trick gesture (direction pressed before the trick button) now fires a marker on the
  press — Up teleports. Not guarded against; written down here as the cost of no hold.
- The HUD change (banner without the bar) was not looked at on screen: no screenshot taken, it is a
  removed fill and a shorter panel.
- Online: `Rides::step` applies the same mask on every peer, so it stays deterministic, but no live
  two-machine ride was run for this task.

## Next

Nothing.
