# 1 — keyboard jk gas brake (FINAL)

Task P80. 2026-10-23.

## What I did

- The new layout's keyboard (`pad.rs` `KEYBOARD_NEW`) puts the gas and the brake on the home row:
  **J is R2** (the gas) and **K is L2** (the brake), with **Space** and **Shift** (either) kept as
  second keys for them. J rides the throttle ramp exactly as Space does: `Bindings::gas(New)` is R2,
  and the ramp reads every key on R2's line, so nothing new was needed for it.
- The face buttons, all on the same row: **H Square** and **L Circle** stay where P75 put them,
  either side of the gas and brake; **Cross moves to G** and **Triangle to `;`** (Semicolon), the two
  home-row keys just beyond HJKL. Neither key was bound under the new layout or used by the shell
  (the shell's own keys are Escape, F3, H where unbound, and M). Why this pair: Triangle is the new
  scheme's Trick 2 and needs to be under a finger, and `;` is the right hand's home-row pinky. Cross
  does nothing in a ride without MX vs. ATV's tricks, so G (left index reach) is enough for it.
- What this changes for the menus: under the new layout the keyboard's Cross (the menus' OK) is G
  now, not J. Enter is Start, which every screen also takes as OK, so the menus are unaffected in
  practice.
- `window.rs` gained `key::G` and `key::SEMICOLON`. The Controls screen labels Semicolon as `;`
  (`key_label`), so its keyboard list now reads Gas "J Space", Brake "K Shift", Trick 2 ";". That
  list is built from the bindings (`keyboard_layout_on`), so `menu.rs` itself needed no change.
- Updated the bindings file's header comment (`Bindings::text`), the doc comments on
  `KEYBOARD_NEW`, `KEY_STICKS_NEW` and `Bindings`, and main.rs's "no pad" line and `--help` text.
- **The old `context/controls.txt`:** the brief says it overrides both layouts. On this branch it no
  longer does: P54's `Bindings::as_written` already reads any line that is exactly the original's
  default (or the pre-P40 select/start) as no line at all, so an unedited written-out file is the
  same as no file. P80 had not been updated since that landed. I did not change that logic. I proved
  it holds for the user's file under the new keys (below). The file has
  `stick_left = J` and `stick_down = K`, which are the original's right stick; neither moves J or K
  under the new layout.
- P75's test `the_new_layout_s_keyboard_is_wasd_and_hjkl` is updated for the new keys. Under the new
  scheme it checks that J and Space are the ramped gas (full when held, part throttle when stabbed,
  no stick moved), that K and both Shifts are the brake, that G/H/L/`;` are Cross/Square/Circle/
  Triangle, and that G and H stay out of a ride.
- `an_old_controls_file_keeps_each_layout_s_keys` gained a check that this machine's old file (its
  exact text, compared line for line against the main checkout's `context/controls.txt`) leaves J the
  gas, K the brake and G/`;`/H/L the face buttons under the new layout. Under both layouts every
  keycode 0–255 reads the same on that file as on no file. Its "the player moved L1" check now uses
  R, because G is Cross now and the check needs a key no layout binds.
- opentennis: nothing to take; its keyboard is gilrs's and a different game's layout.

## What the original does

Nothing to port: the disc reads a pad, and the keyboard is the port's own (front-end, "yours to
design"). The original's layout (`KEYBOARD`) is untouched.

## How it was proved

- `pad.rs` unit tests, run in the `openmx` binary (`./kit check -- -p openmx --bin openmx`: 124
  passed):
  - `the_new_layout_s_keyboard_is_wasd_and_hjkl`: J reaches the ride as the gas and K as the brake,
    and all four face buttons are reachable.
  - `the_original_layout_s_keyboard_maps_exactly_as_before`: the original's keyboard, every keycode, against
    literal keycodes. Unchanged and passing.
  - `an_old_controls_file_keeps_each_layout_s_keys`: an old written-out file no longer moves the new
    layout's keys.
  - `the_controls_screen_lists_the_keyboard`: the Controls screen's list.
- The test's old-file text was checked to be identical, line for line, to the user's
  `context/controls.txt` in the main checkout.

## Not verified / not 1:1

- Not tried on a real keyboard in a live shell window: there is no keystroke injection on this
  desktop (memory: no GUI automation). The keys were proved through `Pads::build` with a fake
  held-key table, which is the same path the X window's held table feeds.
- The user's file was not rewritten. It is read as no file (P54's rule), so its written-out lines
  stay in it. If the user later edits one line to the original's default value on purpose, that line
  still counts as unedited. P54 made that trade-off; P80 keeps it.

## Next

Nothing.
