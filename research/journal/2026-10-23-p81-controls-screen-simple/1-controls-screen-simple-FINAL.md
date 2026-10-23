# 1 — controls screen simple (FINAL)

Task P81. 2026-10-23.

## What I did

P118 had already redone the front end, so the Controls tab (`menu_pages.rs`' `control_rows`) was
what this task cut, not the old seat rows at menu.rs ~:2944 the brief names. Before: Player, Bike
Layout, Controller Preset (dimmed under New), Biplane Layout, Throttle (Cross/R2/L2), Nintendo Pads,
Controller, Pressure Axes. After:

- **Button Layout**, one choice a seat: New, Original, Original, Preset 2/3/4 (`menu::control_choices`).
  P79's presets live inside Original, not as a row of their own; choosing New puts the seat's preset
  back to 0. `controls_label` now numbers presets from 1 as the old preset row did ("Original, Preset 3"
  for preset index 2). Before, it said "Original preset 2" for that one, one off from the row beside it.
- **Plane Stick**: Normal (P69's plane-relative `new`) / Like the Original (`original`). The row's
  description, which the side panel shows under the cursor, follows its value:
  "Normal: down pulls the nose up, always relative to the plane, at every bank and upside down." /
  "Like the original: the disc's own stick, which stops pitching the plane while it is on its side."
  I used the plane-relative wording from the user's correction, not the brief's screen-relative
  one. The "stick turns round upside down" wording in the brief for Original is also wrong: P50's
  research found the disc turns nothing round, and its stick only goes dead on its side. So the hint
  says that.
- **Throttle row removed.** `Gas` loses L2 (user, 2026-10-22): `controls.txt`'s `throttle` line still
  reads `cross` and `r2` and is still written, and an old `throttle = l2` reads as the default (cross).
  `--throttle` takes `cross|r2` and refuses `l2`. `pad::Throttle::Trigger(Control)` stays general
  (pad.rs's own tests wire L2 to prove New's L2 stays the brake). Nothing user-facing reaches it now.
  The pad drawing labels R2 "Gas" under `throttle = r2` and no longer has an L2 case.
- **Player row only with more than one seat** (`Menu::many_seats`: `seated` or `riders` > 1). With
  one seat the tab is 1P's (`Menu::control_seat`), and the keyboard legend that used to hang off the
  Player row shows under Button Layout instead.

Nothing from opentennis: its controls screen has no per-seat layout or plane stick to copy.

### Why the user saw "plane flips"

It was the label, not their file or the default. `controls.txt` in the main checkout (2026-10-20)
has no `biplane` line, so the default is `new`, and `new` maps to `PlaneScheme::New` (plane-relative,
no flip) everywhere: `Front::wiring`, the seats, a tape's playback. P50's seat row (19903df) called its
two plane choices "plane steady" (New) and "plane flips" (Original). It meant "the original's controls
seem to turn round on screen upside down", but it reads as "this setting flips the plane". The row also
stepped through every (layout, preset, plane) combination, so one press on it from the default
showed "New, plane flips". P69 renamed it "plane relative"/"plane original" and P118 split it into
"Biplane Layout New/Original". The fix here is the wording: the default row reads "Plane Stick
Normal", and the other choice is named for what it is.

## What the original does

Menus are ours (AGENT.md). The plane's input behaviour is P50/P69's, unchanged here.

## How it was proved

`crates/openmx/tests/menu.rs`:
- `the_controls_tab_is_one_choice_a_seat_and_a_plane_stick` (new): a fresh file shows Button Layout
  New and Plane Stick Normal, with every seat's plane on `new`. The rows are exactly Button Layout,
  Plane Stick, Nintendo, Controller, Pressure: no Player row with one seat, and no throttle row. Both
  rows step by pad input and save through `menu::save`/`load`. The plane row's hint changes with its
  value and comes back. With two seats the Player row is first and the rows change 2P's.
- `the_controls_row_offers_the_originals_presets`: rewritten for the presets inside Original. It walks
  the five choices round, and 2P's are untouched.
- The P36 save test, the old-file parse test (`throttle = l2` → default) and the tab walk are updated.
- `./kit check -p openmx --test menu`: passed. The full gate: `./kit done`.
- I looked at a `--menu --screen controls --shot` of the tab at 1920x1080: five rows, the preset
  choices and their short tips under Button Layout, and the keyboard legend.

## Not verified / not 1:1

- The plane row's side panel (its hint and tips) was not looked at in a picture: `--screen controls`
  opens on the first row and nothing on the command line moves the cursor. The test checks the hint's
  text, not how it is drawn.
- "One row per seat only when more than one seat is set up" is read as the P118 Player row being
  hidden with one seat, not as one row per seat stacked on the page. P118's tab shows one seat at a
  time through the Player row, and stacking eight seats' rows would undo that redesign.
- Hand-written files that say `throttle = l2` silently go back to Cross gas. There is no notice in
  the front end, only the parse.

## Next

-
