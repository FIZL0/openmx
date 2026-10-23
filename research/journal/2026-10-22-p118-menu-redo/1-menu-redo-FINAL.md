# 1 — menu redo (FINAL)

Task P118. 2026-10-22.

## What I did

- **Ink** (`crates/openmx/src/menu_art.rs`, landed in 2dbac5a): M PLUS Rounded 1c ExtraBold/Bold
  (OFL, `crates/openmx/assets/fonts`) turned into one distance-field atlas at load; a glyph is a quad
  sheared by `SLANT` (tan 11°). Shapes (stadiums, rings, needles, boxes) are `Shape::Round` sprites in
  frame pixels. `raster()` draws the same sprites on the CPU that `game_render::gpu` draws, so the two
  paths are one layout.
- **Painter** (`menu_screens.rs`, new): a `Pen` on an 80×45 em stage (em = min(w/80, h/45)), one
  function per screen. The pause dims the ride (`PAUSE_DIM`) instead of the backdrop.
- **Pages** (`menu_pages.rs`, new): every option page is a `Vec<Row>` (key, kind, label, value,
  choices, about, `off` reason, meter, note) built from the settings, and one `change()` turns a press
  into a setting. Settings' tabs: Ride (Start On, Wrecks, Machines at Bays, Game Modifiers ▸), Display
  (HUD, Speedometer, Draw Distance, Render Scale, VSync, Frame Limit), Audio (preset, six sliders,
  Music Plays, Song, Skip Song), Controls (Player, Bike Layout, Controller Preset, Biplane Layout,
  Throttle, Nintendo Pads, Controller, Pressure Axes, with a pad diagram), Camera (Player, five sliders,
  Default, Original). Garage: Rider (Player, Name, Number, Rider, Colours, Jersey & Pants, Save as
  Preset), Bike (Player, Class, Model, Livery, Tier, Tuning), Gear (Player, the four pieces). Mods: the
  folder's switch and a switch per pack (`settings.mods_off`, new; the Free Ride list honours it).
- **Menu** (`menu.rs`): Main gains Mods; Rider Name moves to the Garage (typed on the menu keyboard);
  the pause's Audio row opens Settings at the Audio tab; Controls/Camera/Audio/Modifiers screens open
  their tab or page. On Free Ride, up/down walk the tracks, L1/R1 pick the setup strip's seat,
  Square/left/right turn that seat's machine, Triangle turns wrecks.
- Old painter (bitmap font, stretched pill textures) removed.
- `tests/menu.rs` rewritten for the new layout, plus `l1_and_r1_walk_every_tab_round_from_either_end`,
  `every_row_of_every_settings_tab_and_page_changes_by_pad`,
  `the_pauses_audio_row_opens_settings_on_its_audio_tab`,
  `a_settings_file_from_before_the_redo_reads_as_the_same_settings`,
  `a_label_at_1080p_and_4k_has_edges_no_wider_than_a_pixel`.
- `tests/hud.rs`' pause test now looks for the HUD above the pause's band (see below).

## What the original does

Not a port of the original's front end: the concept is the port's own design. Kept from the original:
its backdrop, radial glow, logo and track stills (read from the disc), its audio mixes, its four
controller presets, and the pause's machine switch.

## How it was proved

- `./kit check`: TESTS PASSED (2026-10-22, the full gate).
- Every screen and tab is reached by presses alone, and every selectable row of every Settings tab,
  the Modifiers page and the Mods page changes something by Right or Cross, in a top-down and a
  bottom-up walk (`every_row_changes`). Dimmed rows must say why and ignore both presses.
- Old files: a settings.txt with every pre-redo key parses to the expected `Settings` and
  `Settings::text()` writes it back byte for byte; same for controls.txt.
- Sharpness: a slanted Heavy title, a Bold label and a stadium, white on black, at 1080p and 2160p.
  Of the part-covered pixels, the share that do not touch both a ≥230 and a ≤25 pixel in their 3×3:
  1080p 0.88 % (3295 edge pixels), 4K 0.60 % (6617), the same on the GPU (headless wgpu) to the
  pixel count ±6; a 3×3-blurred copy scores 95–96 %. The test bounds it at 3 %.
- CPU vs GPU, every screen at 1280×896 and 1600×720: ≤0.034 apart on average, 0.0000 % of pixels off
  (`the_gpu_draws_every_screen_as_the_cpu_does`, run with the disc; not skipped).
- Looked right beside the concept: `--screen NAME --shot` at 1920×1080 for splash, main, tracks,
  garage, settings, controls, camera, audio, modifiers, mods, keyboard, pause, online, games, lobby,
  join, split in `context/shots/P118/` (not in git); the concept's screens (headless chromium on
  `docs/menu-concept.html#NAME`, stage cropped) beside them, concept left and port right, in
  `context/shots/P118/side/` for main, tracks, garage, settings, modifiers, mods, pause, online and
  local game (join).

## Not verified / not 1:1

- **Logo and backdrop are the disc's** (MX Unleashed logo, `background`, the radial glow), not the
  concept's "openMX" wordmark: the brief's look names "logo", and the disc's is what the front end
  always drew.
- **Pause order**: Clear Markers sits before Main Menu, and Open to Online / Seats are kept, so the
  pause has up to ten rows; the list shrinks its pitch to stay clear of the slanted title. The
  concept's pause chips (Skip song, Change camera) are not drawn.
- **The pause's band covers the bottom of the HUD**, as the concept's chrome does. `tests/hud.rs`'
  `the_pause_menu_keeps_the_hud_under_it` used to want a tenth of the readout box drawn on; it now
  wants a twentieth of the readout and radar boxes above the band (y < 338 at 640×448). Measured 18 %
  and 12 %; a replaced HUD (the bug that test is for) would be 0.
- **Player rows offer 1P–4P only**, not the concept's per-seat "pad 1 · name" notes for every seat.
- **Start On sets 1P's machine only**; other seats' machines are set on Free Ride's setup strip.
- **Controller and Pressure Axes** offer First Found/None plus the one value controls.txt names; they
  are dimmed for players other than 1P ("Other players ride the pad they joined on").
- **Other seats' pads no longer drive the Garage**: the Player row picks the seat (the old per-seat
  garage columns are gone).
- **Skip Song** added to the Audio tab; **Delete Preset** kept on the Modifiers page; music packs are
  not listed on the Mods page (only track packs, by id prefix).
- **Free Ride's setup strip** shows the modifiers but does not change them (Game Modifiers is on the
  Ride tab); Triangle toggles wrecks. Tracks are a vertical list under "MX Unleashed" and "Mods"
  headings, as the concept, but this slot has no mods folder so the shot shows the disc's five and the
  Mods page shot shows "Nothing in the mods folder".
- **Garage Name and Number** are typed on the menu keyboard; the number plate preview is not rotated
  as the concept's is.
- **Controls, Audio, Camera** stay `Screen`s only as stubs that open their Settings tab (the shell's
  `--screen` words still work). There is no `--screen` word for the Display tab, so it has no shot of
  its own; it is covered by the walk and every-row tests.
- **The pad diagram** on the Controls tab is drawn from stadiums and discs, approximating the
  concept's.
- **The modifiers' meters** run from each knob's least to the shared most, so 100 % lights fewer
  ticks than the concept's (which centres 100 %).
- **The modifiers' "takes when the pause closes" note** is on the Player row's note rather than a
  banner.
- **The old "pill as wide as its label" invariant** is dropped for option rows, whose pills span the
  row; main-list pills still fit their label (`every_pill_fits_its_label_and_nothing_covers_the_logo`).
- The 4K sharpness test measures a sample label and stadium, not every screen at 4K.
- Not looked at on a real 4K display, nor with a real pad; the shots are headless.

## Next

none
