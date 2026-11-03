# 1 — the Tricks page (FINAL)

Task P259. 2026-11-03.

## What I did

- `crates/openmx/src/menu_tricks.rs` (mounted as `menu::trick_list`). `rows(set, moves)` builds the list:
  - the 24 gesture slots in slot order, skipping a slot with no name, each with its buttons
    (Circle, Triangle, R1) and stick direction;
  - BackFlip (stick Up, then Down, off a lip, 10,000), Wheelie (Down) and Stoppie (Up), both on the
    ground at 1,000/s;
  - each pack extra on its gesture (Cross, Square or both, over the three);
  - with a pack's `Moves`: Frontflip (R1 + Down, then Up), 360 Left and 360 Right (R1 + one side, then
    the other) and Whip (Left, then Right).
  - A stunt's points come from the ride's own rule: 2,500 a second over its length, floored to 10, with a
    "+" when holding it scores more. The pack moves show their `.phy` points.
- `menu.rs`: `Screen::Tricks` (word `tricks`, so `--screen tricks` works; it starts a ride as `pause`
  does), the pause row `Tricks`, `trick_rows`, `trick_lit()` and `trick_stage()`. Circle goes back to the
  pause, on the row it left. Start resumes. Up and Down walk the list.
- `menu_screens.rs`: the page draws a list panel (TRICK / BUTTONS / POINTS, scrolled to the lit row, a rule
  where the list changes from the original's to the packs' to R1's), the stage box, and under it the lit
  trick's name, where it starts, its gesture in words and its points. New `Btn` glyphs: R1 and the four
  directions.
- `trick_view.rs` (main.rs only): the stage's rider. The first time a view shows the page, it loads its
  own `Body` and `Chassis` off the ride's archives, using the ride's recipe and the packs' clips. Each
  frame it poses them for each paused view's lit trick:
  - the clip under the trick's animation state (gesture slot, `backflip`, `PACK_EXTRA + k`,
    `threesixtyL/R`) on the rider, and on the bike where the bike has it;
  - otherwise the bike stays at rest with the hands kept on the bars (MD41's `bare`).
  - The flips, 360s, whip, wheelie and stoppie also turn the whole bike.
  - It draws with the ride's own models: `render_over` after each of the ride's three GPU renders
    (window, offscreen, shot), and in software into skinned copies after the views are put in the frame.
- main.rs sets `menu.trick_rows` after the load, prints `tricks page: N tricks`, and wires the above.
- README's Controls mentions the page.

## What the original does

It has no such page. Its pause has no trick list. Everything here is an addition. It reads the ride's
tables and clips but writes nothing the ride or a replay reads. The page's body and chassis are its own,
not the seats' rigs.

## How it was checked

- `./kit check -- -p openmx --test menu`:
  - new `the_pause_lists_the_tricks`: the row opens the page on the first trick, Down lights the next,
    Circle goes back to the pause on Tricks, Start resumes.
  - new `over_a_split_the_tricks_page_is_the_pausers`: 2P's page hears 2P's pad alone while 1P rides on.
  - `trick_list`'s 4 unit tests: each group's glyphs match `trick::extra_group_name`; the original's 24
    in slot order with their gestures and points; pack rows and moves; thousands marks.
  - The pause tap counts and row lists in the existing tests are updated for the new row.
  - Everything else in the file passes except `original_on_sets_every_sub_switch_and_one_turned_back_leaves_the_rest`.
    That test fails on Fog ("Whole Map" against "Farther Fog"). It comes from P268's fog change and is
    not touched here; filed as a follow-up.
- `./kit check -- -p openmx --test lobby --test cursor --test machines_everywhere`: 298 passed.
- `./kit check -- -p openmx --bin openmx trick_view`: the bike's turns (a flip is upside down halfway and
  back round at the end; a wheelie lifts the nose) and the pose clip names.
- The browser build compiles (`cargo check -p openmx --lib --target wasm32-unknown-unknown`).
- On screen (release build, private headless weston, free06, 1280x720, `--screen tricks --shot`):
  - Taunt at frames 50, 70 and 90 plays its clip.
  - The software renderer shows the same.
  - `--players 2 --split v` puts the page and its stage in 1P's half.
  - With `--tricks mxvatv`, the list has 50 rows. BackFlip, Frontflip, 360 Left, Whip, Wheelie and a pack
    extra (Rock Solid) each play.
  - These rows were reached with a temporary cursor override, removed before the commit.

## Not verified / not 1:1

- The page is new. Its look is the port's menus', not anything of the original's.
- The whole-bike motion is illustrative and made by eye, not the ride's physics:
  - a flip is one even turn about the bike's middle;
  - a 360 is one even yaw, and its direction on screen was not checked against the ride's;
  - the whip lays the bike over 1.2 rad and back;
  - a wheelie or stoppie tilts 0.45 rad.
- The rider's pose for the moves with no clip is a guess from the ride's pose grid:
  - the whip rides in `air_Mid`;
  - the front flip in `air_Fwd` (as P159 leans it);
  - the wheelie in `turn_riding_Bck`;
  - the stoppie in `turn_riding_Fwd`.
- The whip's gesture text (stick Left, then Right) and the wheelie/stoppie rows are the port's reading of
  the ride's code. They were not checked against a pad on the original.
- Every split view's stage shows the first player's rider and bike, not each seat's own pick.
- The live view plays the clip by phase, with no cross-fade in or out. The bike's drives see no
  suspension travel or wheel spin.
- No software still is needed: both renderers draw the trick live. The brief's still picture is the dark
  stage box. It only shows when neither a rider nor a bike loads, and that case was not seen here.
- Keyboard: the page uses the menu's own Up/Down/Circle/Start mapping, which the existing keyboard tests
  cover. No keyboard-only run of this page was made.
