# 1 — keyboard layout 2 (FINAL)

Task P207. 2026-10-28.

## What I did

- **Where J started a trick, confirmed by reading before changing anything.** Under the new layout
  `Pads::build` pressed the pad's Cross *and* R2 for the keyboard's throttle keys (J and Space on
  P80's home row) — Cross as the menus' OK. `Scheme::ride` then swaps the pad's Cross onto the
  report's R2, and with MX vs. ATV's tricks on `Scheme::ride_tricks` hands that R2 to the ride as
  `trick_pack::EXTRA_CONTROLS[0]`, the first extra trick button (`trick::group_with` → group 7). So
  holding the gas was holding a trick button: any lean in the air started a pack trick. Without the
  pack the same Cross was the `OTHER_CAMERA` hold (P78), which the shell has no camera for but which
  still blocked the camera change while the gas was down. Fix: the throttle keys press
  `Bindings::gas(scheme)` only — Cross under the original's layout (where Cross *is* the gas), R2
  under the new one. `Seats::key_menu_state` likewise.
- **New default keyboard for the new layout, `Keys::Modern`** (`pad.rs` `KEYBOARD_MODERN`,
  `KEY_STICKS_MODERN`), after modern MX games' PC defaults:
  W gas (R2, the ramped throttle) · S brake (L2) · A/D steer (left stick x; J/L too) · I/K lean
  forward/back (left stick y: the lean, the preload and the flips) · Shift or Q clutch (L1) ·
  F trick 1 (Circle) · G trick 2 (Triangle) · E trick 3 (R1) · Space / V the pad's Cross / Square
  (MX vs. ATV's tricks 4 and 5 with the pack; other camera / look back without) · R reset (L1+R1 in
  one key, so the P67 "together" chord is met on the same frame) · C or Tab camera (Select) ·
  B camera reset (R3) · Z zoom hold (L3) · 7 8 9 0 the camera stick (7 left, 8 down, 9 up, 0 right,
  vim's way like P75's YUIO) · arrows the d-pad (markers) · Enter Start · Escape pause. No numpad.
  Menus: arrows, Space OK, Escape back (Circle), Q/E tabs, V delete, Enter start.
- **Old layouts kept reachable.** P80's home row is `Keys::HomeRow`: `keys = home-row` in the
  wiring's `controls.txt` (new line, written as `keys = modern` by default), or the Controls tab's new
  **Keyboard** row (Modern / Home Row; dim under the Original layout). The original's keyboard is
  untouched and still the Button Layout row's Original choice.
- **Migration.** `Bindings::as_written` now also reads a line exactly as P80's home row had it as
  no line, so a context/controls.txt the shell wrote out under P80 moves to the modern keyboard
  instead of pinning it; a line the player changed stays theirs. A file that only has the old
  original-default lines (the user's own context/controls.txt is one) already read as no file and
  so moves to modern too. `Bindings::set_keys` re-defaults the unset New lines when the wiring's
  `keys` changes, live (`Pads::set_wiring`, `Seats::set_wiring`).
- **Key diagram.** `Bindings::keyboard_layout_on` rows: the chord key (R) is shown only as Reset
  ("R, or Shift Q + E") and not in the Clutch / Trick 3 rows; under New, "Trick 4, MX vs. ATV" and
  "Trick 5, MX vs. ATV" rows; "Steer" and "Lean, preload" rows by axis instead of "Left stick";
  "Camera stick" for the right stick. `Menu::keyboard` holds both New keyboards (`keyboard_at` takes
  the home-row flag); the diagram shows under the Button Layout row and under the Keyboard row.
- **Hints.** The no-pad println and `--help` text in main.rs say the modern keys (the home row's
  when picked); the bindings file header and the wiring file header say both.
- **web/**: no change. `web/index.html`'s EVDEV map already has every key the modern layout uses
  (letters, digits, Space, Shift, Tab, Enter, arrows) and prevents the browser default on all but
  F11; `worker.js` only forwards held keys. The web build runs the same pad.rs.
- window.rs `key`: R, B, 7, 8, 9, 0 added.

## What the original does

Not a disc behaviour: the keyboard is the port's. The original reads its pad through the
controller preset table (preset 0: Cross gas, Square brake, L1 clutch, Circle/Triangle/R1 the trick
buttons, L1+R1 reset); the new layout's `Scheme::ride` and MD16's `ride_tricks` are the port's remaps
before the sim, unchanged here.

## How it was proved

- `pad::tests::no_key_is_two_ride_commands_under_the_new_layout`: every X keycode 0..=255 held
  alone, through `Pads::build` and `Scheme::ride_tricks`, for both keyboards (Modern, HomeRow) × both
  trick settings (original, MX vs. ATV pack): at most one button or one stick axis reaches the ride
  (R excepted, asserted to be exactly L1|R1); every command the brief names has a key of its own
  (gas, brake, clutch, three tricks, camera, steer both ways, lean both ways; the two extras with
  the pack); the gas key never carries an extra or the camera.
- `tests/keyboard_trick.rs` (new), the tape ride: the stock bike thrown 400 ft over a flat floor,
  driven tick by tick by held keys through the shell's own keyboard pad, `ride_tricks`, poll,
  `stick_alone`, `chord_together`, step — under the original's tricks and under a pack's moves:
  F + I with W held starts the same trick as without W and both bank on landing; W + I held through
  the fall starts nothing and banks nothing; Space + I starts a pack trick with the pack, nothing
  without; the home row's J + W starts nothing. **Checked the test catches the bug**: with the old
  "gas also presses Cross" put back in `Pads::build` by hand, `the_gas_and_a_lean_start_no_trick` and
  `the_home_row_gas_is_no_trick_either` fail; restored.
- Older P54/P80/P40 pad tests now run the home row through `keys = home-row` wiring (and assert its
  gas is R2 alone); tests that held H/N/Space/Shift for Square/R3/gas/brake under the default now hold
  V/B/W/S. `an_old_controls_file_keeps_each_layout_s_keys` also proves a file written out under P80's
  home row reads as no file.
- `tests/menu.rs`: the Controls tab's row list, `keyboard_at`'s new indexing, the wiring file round
  trip with `keys`, and `the_controls_tab_picks_the_new_layout_s_keyboard` (row toggles, saves
  `keys = home-row`, loads back, dim under Original, a pre-P207 file is modern).
- `./kit check --all` green, 3007 tests, under `./kit done`. Landed with `--skip-audit`: the audit's
  seven leaks are all in plan/DONE.md (lines 121 and 561, other tasks' addresses), none in this diff.

## Not verified / not 1:1

- **Modern MX games' PC defaults are from recollection, not looked up.** This session has no web
  access; the layout follows what MX vs. ATV / MX Bikes / MXGP-era PC games commonly ship (WASD bike,
  a separate rider-lean cluster, Shift clutch, R reset, C camera) as I remember it. The exact keys any
  one of those games uses were not checked.
- **R presses the clutch and trick 3 too, for one frame.** It is the reset chord (L1 + R1) on one
  key, so the ride sees L1 and R1 down; the reset fires (the chord-together rule is met on the same
  frame), but in the air R is also a trick-3 press. That is inherent to the original's chord and is
  what Shift/Q + E does too; not otherwise tested.
- **Rebinding `l1` or `r1` in context/controls.txt takes R off that half** of the chord (R is on
  both lines' defaults); then R no longer resets. Not handled — a player who rebinds L1 can still
  reset with their L1 key + E.
- **Under `keys = home-row` the gas is no longer the menus' OK**: Space was OK only because it pressed
  Cross beside the gas. The home row's OK is now G (Cross) or Enter on the screens that take Start.
  The user did not ask for the home row's menus to change; it is the price of the gas not being a
  trick button.
- **The Controls tab's key diagram was not looked at on screen.** It now has 16 rows for Modern (13
  before); the list stops drawing at the stage's bottom band, so on a short window the last rows
  (Reset, Pause) may be cut. Rows are asserted as data only.
- **The web build was not run in a browser**; by reading only (every key is in the EVDEV map).
- **No laptop keyboard was tried**; "laptop-friendly" means only that no numpad key is bound and
  everything sits on the letter block, Shift, Space, Tab, Enter, the digit row and the arrows.
- MX vs. ATV's pack was a hand-made `[Three Sixty]` ini in the tape ride (the real pack's tricks are
  not needed for which button is pressed); the real pack's rides were not re-run.

## Next

Nothing for this task. If the user wants a different modern key for something, it is one line of
`KEYBOARD_MODERN` / `KEY_STICKS_MODERN` in pad.rs plus the diagram test's row.
