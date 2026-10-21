# 1 — keyboard hjkl face buttons (FINAL)

Task P75. 2026-10-21.

## What I did

- Under the new layout (`KEYBOARD_NEW` in `crates/openmx/src/pad.rs`) H/J/K/L are now the face buttons,
  vim's left/down/up/right on the diamond: H Square, J Cross, K Triangle, L Circle. They used to be the
  right stick (P54).
- **The right stick is on Y/U/I/O** (`KEY_STICKS_NEW`): Y left, U down, I up, O right. That is the
  row directly above HJKL, in the same vim order, so the right hand only moves up one row. None of the
  four clash with anything in `KEYBOARD_NEW` (Space, Shifts, Q, E, Tab, C, Enter, arrows, HJKL, Z, N)
  or with WASD. Why not the 1-4 row the brief offers: it sits above the left hand's WASD, so you would
  have to let go of the left stick to reach the right one. Why not U/I/O/P: it is one key further from
  the HJKL fingers.
- The 1-4 row is no longer bound under the new layout. P54 had the face buttons there, and HJKL takes
  that job over. `key::ONE..FOUR` are gone from `window.rs`. `key::Y/U/O` are added.
- The original layout is unchanged: `KEYBOARD` and `KEY_STICKS` (IJKL) were not touched, and P54's
  literal-keycode test `the_original_layout_s_keyboard_maps_exactly_as_before` still passes.
- Updated the comment in `controls.txt` (`Bindings::text`), the doc comments, the shell's `--help` text
  and the "no pad" line in `main.rs`, and the overlay's "H, where H is not a stick key" wording (H is
  still a key of the ride under the new layout, so the overlay stays on F3 alone there, as before).
- P36's Controls screen (`menu.rs`) builds its keyboard column from `Bindings::keyboard_layout`, so it
  picks up the new keys without any change to `menu.rs`. Its test `the_controls_screen_lists_the_keyboard`
  now expects Trick 1 = L, Trick 2 = K, Right stick = "I Y U O".
- Tests: `the_new_layout_s_keyboard_is_wasd_and_hjkl` now holds each of H/J/K/L and checks it presses
  its face button and leaves all four stick axes centred. It also checks that Y/O/I/U push the right
  stick to its four edges and press no button. The seat tests that used `key::THREE` as the keyboard's
  Square now use `key::H`.
- opentennis: nothing to take, since its keyboard map is its own game's.

## What the original does

Nothing to port: the keyboard layout is front end (the PS2 has no keyboard).

## How it was proved

Unit tests in pad.rs, run through `./kit check` (TESTS PASSED).

## Not verified / not 1:1

- I did not try the new keys by hand in a running shell window. No keystrokes reach windows on this
  desktop, so the proof is the unit tests on `Pads::build` and `Bindings` alone.
- A `controls.txt` that a player edited by hand to bind lines to the 1-4 row or HJKL still overrides
  these defaults under both layouts, as P54 designed. A file whose only lines are the shell's commented
  defaults is unaffected.

## Next

none
