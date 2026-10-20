# 1 — p36 a front end (FINAL)

Task P36. 2026-10-20.

## What I did

- **`crates/openmx/src/menu.rs`**, the port's front end, in two halves:
  - A pure state machine: `Reader` turns one pad report a frame into a `Press` (edges only, d-pad or
    left stick past 64 of 127, a held direction repeating after 20 frames then every 5, and the
    first report a fresh reader sees taken as already held so the Cross that opened a screen does
    not press on it too). `Menu::step(Press) -> Act` walks a stack of screens: **Main** (Free Ride,
    Settings, Quit — P37's garage and P38's join screen go in `MAIN`), **Tracks** (the five freeride
    tracks, left/right to pick, Cross rides), **Settings** (Name on the keyboard, Overlay, Wrecks,
    Controls), **Controls** (the throttle: Cross, R2 or L2), **Keyboard** and **Pause** (Resume,
    Restart, Settings, Main Menu, Quit). In a ride with nothing open the only thing it hears is Start.
  - The settings in `settings.txt` (name, overlay, wrecks, last track) and `controls.txt` (throttle,
    pad device, `--pad-pressure`'s map) beside the disc, `key = value` a line, `#` comments, unknown
    keys and bad values falling back to the defaults.
  - The drawing: `Art::load` reads the menus' art off the disc and `Art::draw` paints the screen into
    the shell's software `Frame` in the PS2's 640x448 layout, scaled to the frame, with bilinear
    filtering and a tint per draw. Text is the disc's own Yearling fonts (`font\eng\*.cel` + atlas).
- **`crates/openmx/src/main.rs`**: the old `run` is now `ride(args, shell)`, unchanged for a ride with
  no front end. With no flag that picks a ride (`--track`, `--play`, `--record`, `--moment`,
  `--vehicle`, `--phy`, any `--trace*`, or the new `--ride`) the shell opens on the front end;
  `--menu` forces it and `--screen NAME` opens on one screen (and for `pause`, over a ride on the last
  track), which is how the shots were taken. The window and pad are opened once and lent to each
  ride; Start or Escape in the ride opens the pause instead of the plain freeze (a new
  `World::start_freezes`, false under the front end), the pause is drawn over the frozen ride, and
  what it changes (overlay, wrecks, throttle) applies at once and is saved. Restart re-runs `ride`
  on the same track; Main Menu returns to the menu. A ride that fails to load puts its error on the
  menu's ticker instead of ending the program. Command-line flags still win over the files for the
  session (`--hud`, `--no-wrecks`, `--pad`, `--throttle`, `--pad-pressure`).
- **`crates/openmx/src/pad.rs`**: `Pads::set_wiring`, so the Controls screen rewires the pad live.
- **`crates/game-render/src/disc.rs`**: `Disc::configured_path`, where the disc is (settings go beside it).
- **`crates/game-render/src/pst.rs`** — two decoder fixes the menu art needed, both outside the menu:
  - the sixteen-colour `.pst` (second header word 0x2e, a 16-entry palette in order at 8, the header
    at 72, pixels at 92, two a byte low nibble first), which is most of `ui\tex\*` and the fonts'
    atlases; `Texture::parse` refused all of them before (it read a mip count of 4294967295);
  - **height before width** in both headers. The title screen's texture says 448 then 640, and
    `free06`'s bushes billboard is a clean strip of plants read as 512x256 and alternating cut rows
    read the other way. Square textures (most) read the same either way. There are 2211 non-square
    256-colour textures on the disc, so **every track's non-square textures change** with this —
    that is a rendering fix everywhere, not just the menus.
- **`.gitignore`**: `/settings.txt` and `/controls.txt`, because the disc sits in the repo root here,
  so the front end's files land there.
- **`crates/openmx/tests/menu.rs`**: six tests (below).

### From opentennis, and what changed

Taken: the structure of `crates/hst/src/play/main_menu.rs` (a screen enum and a state machine fed
one press a frame, separate from the drawing), the on-screen keyboard's four rows of ten
(`abcdefghij` / `klmnopqrst` / `uvwxyz0123` / `456789-'.&`) with Space, Delete, Clear and Done and
Square as delete, Circle to leave, and `hst-base`'s `store`: `settings.txt` beside the ISO (its parent
directory, or `.`), parsed as `key = value` with `#` comments and `on`/`off`.

Changed: opentennis is a Bevy app that runs a match as a child process; openmx is one std window and
one loop, so the ride runs in the same process and the menu paints the same software frame the ride
does. Its Settings struct is a tuple of many fields; ours is four named ones. Its keyboard shows the
typed text in its own font; ours capitalises the first letter of each word as typed.

### The disc's menu art, as read

- `UI\ui.res`: textures `ui\tex\*.pst` (with `.psm` materials beside them, not read), fonts
  `font\eng\yearlingdash*.cel` + `.pst`, layouts `ui\*.lyt`, styles `ui\mxfrontend.sty`.
  `UI\trackpreview.res`: `ui\tex\<track>_preview_01..05.pst`, 256x256 each.
- `.cel`: a 12-byte header (f32 2.71 in every font, u16 point size, s16 -1, u32 glyph count) and then
  per glyph: u32 code, four f32 box corners in the atlas (0..1), the code again, s16 width, s16
  space before, s16 space after, u16 line height, u16 kerning pairs, and the pairs as (u16 next
  code, s16 adjustment). Glyph 0 is the box drawn for a missing code. The advance is
  before + width + after (+ kerning); the width field is the box's width to the texel in every glyph
  looked at.
- `mainmenu.lyt` (in 640x448): the column `mainmenu_background` at X 107, 256x448, its shadow at 117;
  the cursor `mainmenu_highlight` 256x32; items centred on X 185 from Y 100, 30 apart; the logo
  512x64 at (20, 25), its shadow at (30, 35). `mxfrontend.sty`: MenuItem `yearlingdashbold_20` in
  204,204,204 at alpha 210, selected `yearlingdashboldoblique_22` at 255.
  `mxfrontendnavigation.lyt`: the ticker panel at Y 403 (640x45), the navigation panel's corner
  `nav_bkgcorner` and its extension at Y 377, the screen title at (50, 370) left-aligned, the button
  icons `nav_*_off` 64x64. `trackpreview.lyt`: masks over the top 49 and the bottom 90 rows.

## What the original does

Not ported, by design (AGENT.md leaves the front end to us). The main menu in the original
(`context/shots/c1-title.png` in the main checkout, plan/HUMAN.md's main-menu state) is the orange
gradient, the dark column with the logo over it, the red rounded bar under the chosen row in the
oblique face, the other rows in grey, a bike and rider standing on the right, and a black legend in
the bottom right corner ("ⓧ Next") over a black ticker strip. The port draws that from the same
textures and the same layout numbers.

## How it was proved

- `crates/openmx/tests/menu.rs` (6 tests, through `./kit check -- -p openmx --test menu`; then the
  full `./kit check` passed):
  - `every_screen_is_reachable_from_the_main_menu_by_pad_input_alone`: a breadth-first search over
    the eight menu buttons, each a press report and a release report through `Reader`, from a fresh
    main menu, finds all six screens (Pause through Free Ride → Cross → Start in the ride).
  - `the_walk_a_rider_takes_rides_pauses_and_comes_back`: pick Bridgeport by the d-pad and ride it,
    Cross in the ride does nothing to the menu, Start pauses, Settings from the pause and back,
    Restart, Main Menu, Circle resumes.
  - `a_held_direction_repeats_and_the_stick_counts_as_one`: a second of full-right stick steps the
    pick at least three times, a centred stick none, and a fresh reader does not press a held Cross.
  - `the_settings_file_reads_back_as_it_was_written`: non-default settings and controls (a name with
    a space, R2, a device, a pressure map) saved and loaded back equal; a missing file and a file of
    nonsense load the defaults; the defaults round trip.
  - `what_the_pad_sets_is_what_the_file_says`: the name typed on the keyboard by pad (Clear, M, x,
    Square, Start), overlay on, wrecks off, the throttle to L2, saved, loaded back, the same.
  - `every_screen_draws_from_the_disc`: the art loads off the disc and every screen paints over
    90% of a frame. It ran (made to panic on its skip branch once to check; it did not skip).
- By eye, beside the original's main-menu screenshot: `openmx --screen <each> --shot ... --size
  640x448` for all six screens (the shots are in this session's scratchpad, not committed). The
  main menu reads as the original's: same logo, column, bar, fonts, colours and legend.
- The pst height/width fix: by eye on the bushes billboard (512x256 clean, 256x512 cut) and on the
  menu textures; the full check (including the renderer's tests) passed with it.

## Not verified / not 1:1

- **The window loop under the front end was never driven by live input.** This desktop delivers no
  keystrokes and `/dev/uinput` is root-only, so the menu → ride → pause → menu round trip ran only
  as the state machine in the tests and as `--screen pause` (a ride really started from the front
  end with the pause drawn over it). Escape's edge, keyboard Return/arrows in the menu, Restart and
  Main Menu leaving `ride` and coming back, and a ride error landing on the ticker were not run.
- **Every non-square track texture now decodes differently** (height-before-width). Checked by eye on
  one texture (`free06`'s bushes billboard) and on the menu art only; the other 2210 non-square
  256-colour textures on the disc were not looked at one by one.
- **Not 1:1 on purpose**: the menus are the port's (AGENT.md). Screens, rows, wording and flow are
  ours; only the art, fonts and layout numbers are the disc's.
- **The bike and rider standing on the main menu are not drawn** — the 3D turntable is a model and an
  animation the menu does not load. The right half of the main menu is the background only.
- **The background's colour is chosen, not read**: `background.pst` is a grey gradient the original
  tints by a colour animator (`ColorFader` in the layouts), which is not read; `ORANGE` is matched to
  the screenshot by eye, and the radial light's tint and strength likewise. The Flash layers
  (`flash_*.psm`, `.fsh`) and every animation (fades, slides, the cursor's) are not reproduced.
- **Labels and track names are typed in, not read off the disc.** The UI string table
  (`appstart.res` `loc\ui.loc` ids, `loc\l_ui_eng.bin` strings) has 1551 ids and 1581 strings and my
  pairing drifted, so no string comes from the disc. The track names (Dallas, Norfolk, Bridgeport,
  Rochester, Cleveland) are mine from earlier tasks, not checked against the original's track pick.
- **Layouts placed partly by eye**: the item rows, logo and column are `mainmenu.lyt`'s numbers, but
  the highlight's Y (row − 5), its shadow offset (+10, +10), the legend's exact placement and the
  title's Y (362, not the layout's 370, so it clears the ticker) were fitted to the screenshot.
  The Settings, Controls, Keyboard, Tracks and Pause screens have no original counterpart drawn here
  — the keyboard and track pick are laid out by me, the track band uses `trackpreview.lyt`'s masks.
- **`.cel` fonts**: the header's f32 (2.71) and s16 (-1) are unexplained, the line-height field is
  not used, and some hundreds of bytes after the last glyph in each font are not read. The `.psm`
  materials beside every texture are not read (blend modes and alphas are chosen per draw).
- **The keyboard takes pad input only** — typing on the host keyboard is not supported, since the
  shell's keyboard is already the pad's stand-in (X is Cross, Z Square, C Circle).
- **The pause reads pad 0 only**; P38 owns more seats. `--screen pause` pauses from the first frame,
  so the rider's body has not been posed yet and only the bike is drawn under it.
- **The settings files sit beside the disc path as `kit.toml` names it**, i.e. in the repo root here
  (git-ignored now). With the ISO as a symlink, they go beside the link, not beside its target.

## Next

P37 and P38 add their screens to `MAIN` and `Screen` in `menu.rs`; the main loop's `Act::Ride` is where
a picked bike and rider would go into the ride's `Args`.
