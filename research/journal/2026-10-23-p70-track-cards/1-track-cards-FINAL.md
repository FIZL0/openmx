# 1 — track cards (FINAL)

Task P70. 2026-10-23.

## What I did

- **The disc's track art.** Listed `UI\ui.res`, `UI\trackpreview.res` and `UI\loadscrn.res`. The front
  end's track-select art is the five stills a track in `trackpreview.res`
  (`ui\tex\<track>_preview_01..05.pst`, framed by `ui\tex\trackpreview_frame` and laid out by
  `ui\trackpreview.lyt`); there is no smaller per-track icon for the free-ride tracks (the `league_*`
  pictures are the career leagues'). So a disc card's picture is its track's first still, and the
  lit card turns through all five, the way the original's preview turns them. The stills were
  already loaded for P36's single framed still; nothing new is read off the disc.
- **Cards.** `menu_screens.rs`'s `tracks` draws the lit category's tracks as cards (`Pen::cards`)
  in the left column where the rows were: P39's lit pill look for the lit card (the red gradient,
  chalk rim, hard shadow), the glass rows' look for the rest; each a 16:9 picture, the name, and a
  line under it (Last ridden, or where the track is from). As many as fit, the lit one kept in
  view, with the rows' scroll arrows. The big framed still on the right stays and now shows a
  mod's picture too, as do the main menu's Last ridden still and the splash.
- **Navigation unchanged**: Up/Down walk the lit category's cards, Left/Right the categories
  (the user's rule of 2026-10-23 for the list holds for the cards). That is why the cards
  are one column and not a grid: a grid would want Left/Right for itself.
- **Custom tracks** are MD2's (mods/tracks/<id>/, worlds included). Their picture
  (`track_previews.rs`, new): the mod's own `previews/track.png` (the openmx-mods standard T1 §2's
  optional file, whose size it left to P70: any 8-bit non-interlaced PNG up to 2048 a side, shown
  cropped to 16:9 about its middle), else a still from its spawn: the shell's own binary run
  `--headless --track <id> --mods <folder> --frames 1 --size 512x288 --overlay none --shot`, in a
  background thread so the menu never waits (~0.8 s a mod), written to a `.part.png` and moved
  over, kept in `mods/previews/<id>.png` (beside `tracks/`, which the scan does not read) and taken
  again when the mod's `mod.json` is newer. The card has no picture until it lands, then the shell
  gives it to `Art::set_mod_picture` and the GPU its images again.
- **PNG reader** (`track_previews::decode_png`): the workspace has no PNG crate and wants none;
  8-bit grey, grey+alpha, RGB, RGBA and palette, all five row filters, `tRNS` for palette, grey and
  RGB; inflate is game-sim's own `archive::inflate::zlib`.
- `Art::images` now counts the mods' pictures after the stills; `Art::track_image(id, i)` says which
  picture a card shows; `Part::Card` says where each card and its picture went, for the test.
- stages.rs (on the task's line) is the frame stopwatch and has nothing of the pick in it; not
  touched.
- Took nothing from opentennis: it has no track pick with pictures.

## What the original does

Its track select shows one track's preview at a time in `trackpreview.lyt`'s frame, turning
through that track's five stills. The menus are ours to design (AGENT.md), so the cards are not
the original's layout, only its pictures.

## How it was proved

- `tests/menu.rs` `the_pick_is_a_card_a_track_with_its_own_picture` (disc): at 640x448, 1080p,
  21:9 and portrait, the pick draws a card for each of the five disc tracks in order, each card's
  picture is byte-for-byte that track's `_preview_01` off the disc, 16:9 and inside its card, the
  cards do not overlap, the lit one turns to `_preview_02` after 150 frames while an unlit one stays;
  a mod category's cards show the picture given to the art, none before one is given, and a later
  one from then on. Proved to run on the disc (made it panic at its end once: every assert before
  passed). `every_pill_fits_its_label_and_nothing_covers_the_logo` also checks no card meets the
  logo.
- `tests/track_mods.rs` `a_custom_track_shows_its_own_picture_or_a_still_from_its_spawn` (disc): a
  generated mod's own `previews/track.png` is read back pixel for pixel; one without is the only one
  queued; the shell's binary takes its still at 512x288 (not blank, kept, no `.part` left); the scan
  still lists exactly the three mods; the next gather reads it rather than taking it, and takes it
  again once `mod.json` changes; a mod whose ride cannot start is an error, not a crash; a broken
  PNG is one log line. Riding a custom track is MD2's `a_track_mod_loads_and_rides…` (five seconds
  on the throttle), and the still itself is a one-frame ride from the spawn.
- PNG reader against ImageMagick: six files (a 1024x1024 deflate-compressed RGBA icon from
  /usr/share/pixmaps, and RGBA, RGB, palette, grey with `tRNS`, grey+alpha made by `magick` from it)
  decoded identical to `magick -depth 8 RGBA:` byte for byte. Unit tests: the shell's own `--shot`
  PNGs read back; every truncation and every single-byte flip of one is an error or a picture,
  never a panic.
- By eye: `--screen tracks` shots at 1280x720 of the disc pick and of a copy of openmx-mods'
  `mxvatv_fs01` (Little Rock FMX) with its spawn still.

## Not verified / not 1:1

- The menus are ours (AGENT.md), so the card layout matches nothing on the disc by design; only the
  pictures are the original's.
- The GPU path of the cards was not looked at on a window: the shots are the software raster of the
  same sprites (`Art::parts`), and the re-upload of images when a still lands (`menu_images = false`)
  is not exercised by a test — no headless GPU front end here.
- A mod's own `previews/track.png` was only tried with generated and ImageMagick-made PNGs; no mod
  in openmx-mods ships one yet. 16-bit, 1/2/4-bit and interlaced PNGs are refused (one log line),
  not read.
- The spawn still is taken with the user's garage pick and the default follow camera, so two
  machines can take slightly different stills of the same mod; it is a picture, not a hash input.
- A mod whose ride cannot start is tried again (≈1 s in the background) every launch; nothing
  remembers the failure.
- On wasm there is no front end with mods pictures: the job runs the shell's binary, which the
  browser has no way to; the web build has no track pick today.

## Next

MD11 puts its by-game filter on these cards.
