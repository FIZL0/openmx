# 1 — keyboard wasd hjkl (FINAL)

Task P54. 2026-10-21.

## What I did

The keyboard's defaults are now per layout (P40's scheme), all in the shell's input layer
(`crates/openmx/src/pad.rs`). The simulation reads the same pad report as before.

- **New layout** (`KEYBOARD_NEW`, `KEY_STICKS_NEW`): WASD is the left stick (A/D steer, W leans
  forward, i.e. stick up, the low byte; S leans back). HJKL is the right stick, vim-style. Space is the
  gas, ramped (the `r2` line's keys are the throttle under this layout). Left or right Shift is the brake
  (L2). Q/E are L1/R1, Tab or C the camera (Select), Enter Start, and the arrows the d-pad. 1-4 are
  Cross/Circle/Square/Triangle. Z/N are L3/R3. Escape was already the pause under the front end
  (main.rs), so nothing changed there.
- **R and F are not bound** under the new layout. The brief offered them "for L2 and R2 where the
  scheme still needs them", but under the new layout L2 and R2 are the brake and the gas, and those are
  already on Shift and Space. The original layout keeps F=R2 and S=L2 as before.
- **Original layout**: `KEYBOARD`, `KEY_STICKS` (IJKL), and X/Space on the throttle are unchanged. A
  test checks all 256 keycodes against a literal table of the pre-P54 bindings.
- **`controls.txt`**: `Bindings` is now `{ lines: [[Vec<Input>; 24]; 2], set }`, one set of lines per
  layout. A line the file has applies under both layouts. The four new lines, `left_stick_*`, come after
  the old `stick_*` lines, which are still the right stick. The file written on first run has every line
  as a comment showing each layout's default (`# l1 = Q PadL1  # new` / `# l1 = A PadL1  # original`).
- **Old files**: the shell used to write every line in full, including on this machine
  (`context/controls.txt`, which predates P40). A line that matches what the shell wrote before, as a
  set, is read as "no line": the original layout's default, or the pre-P40 `select = Tab` /
  `start = Return PadBack PadStart`. Without this, every existing install would have kept the original
  keys under the new layout too.
- **Which layout the keyboard uses**: in a seat, that seat's layout. In the menus (`Seats::devices`)
  and for `keyboard_alone`, the keyboard's seat's layout, or 1P's. Before, the keyboard in the menus
  always used the original layout, because its `Pads` started at `Scheme::Original`.
- **Overlay key**: H toggled the overlay, but under the new layout H is the right stick. F3 now always
  toggles the overlay. H still does too, but only when the keyboard's layout does not bind H
  (`Seats::binds_key`). M (the machine swap) is free under both layouts.
- **Controls screen** (menu.rs): a column down the right lists the keyboard for the layout of the seat
  row the cursor is on (1P's on the throttle row). It is built from the bindings
  (`Bindings::keyboard_layout`), so a `controls.txt` override shows where the key moved. The menu holds
  plain strings (`Menu::keyboard`), set once in main.rs when the seats open. I added `Align::Right` for
  it.
- Updated text: the `--help` keyboard lines, the "no pad" lines, the bindings file header, and PLAN.md's
  `hud.rs` line.

Taken from opentennis: nothing new. Its controls.rs already gave the per-action `Vec` bindings that
P35 ported. Its WASD is menu navigation, not a stick.

## What the original does

Nothing here. The disc reads a DualShock 2, and the keyboard is the port's own.

## How it was proved

`./kit check` passes. New tests in `pad.rs`:
- `the_new_layout_s_keyboard_is_wasd_and_hjkl`:
  - each of WASD/HJKL moves only its own axis, to the edge, and presses no button;
  - through `Scheme::New.ride` and the game's own poll (`Controls::from_pad`), A/D/W/S give the same
    `stick` as a pad's left stick at those bytes;
  - Space is the gas (a two-frame stab is part throttle, a hold is full), X is nothing, and either Shift
    is the brake with no clutch or front brake;
  - Q/E reach the ride as L1/R1, Tab and C as Select (the new layout's camera), Enter as Start, the
    arrows as the d-pad, 2/4 as Circle/Triangle and Z/N as L3/R3;
  - 1 and 3 are Cross and Square on the pad report (the menus' OK and delete), and the ride never sees
    them;
  - H is bound under new and not under original; M and F3 are bound under neither.
- `the_original_layout_s_keyboard_maps_exactly_as_before`: every keycode 0-255 held alone under the
  original layout gives exactly the report the literal pre-P54 table gives, and `Scheme::Original.ride`
  leaves it alone.
- `an_old_controls_file_keeps_each_layout_s_keys`: this machine's pre-P40 file and the P40-era default
  both parse to `Bindings::default()`. One changed line (`l1 = G`) applies under both layouts. The
  written file comments each layout's default.
- `the_controls_screen_lists_the_keyboard`: the screen's rows under both layouts, and a moved key.
- Existing tests that rode the keyboard through `Seats` under the default (new) layout now use the new
  keys: Space instead of X, Shift instead of S, and 3 instead of Z for Square.

The Controls screen was rendered with `--screen controls --size 640x448 --shot` and looked at. The
column fits beside the list.

## Not verified / not 1:1

- **No live keypress.** No keystrokes reach a window on this desktop
  (memory: no-gui-automation-on-this-desktop). So WASD/HJKL were proved through `Pads::build` with a
  `held` function, not by a person pressing keys in the running shell.
- **WASD on the left stick goes through P30's key ramp** (0.18 s to the edge). P30 found that this ramp
  cannot make a pump-and-flip's three stick crosses inside a lip on the disc's 125, which is why the
  original layout's left stick has no keys. That still holds under the new layout. A keyboard flip
  needs the d-pad (arrows), as before. I did not try to retune the ramp, which is outside this task.
- **The Controls screen under the original layout was not rendered.** Its rows are unit-tested, and I
  checked that 12 rows at 20 px fit above the title by arithmetic. I did not see a screenshot.
- **Choices I made where the brief left room:**
  - Z/N for L3/R3. The brief said only "in the same spirit".
  - Both Shifts are the brake.
  - R/F are left unbound under the new layout (see above).
  - F3 for the overlay.
- **Rare old-file case.** A player whose old `controls.txt` deliberately kept a line identical to the
  old default (for example `l1 = A PadL1`) now gets Q under the new layout for that line. The file
  cannot tell "the shell wrote this" from "the player chose the default".

## Next

None. The task is done.
