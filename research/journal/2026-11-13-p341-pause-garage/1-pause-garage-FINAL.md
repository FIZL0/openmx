# 1 — pause garage (FINAL)

Task P341. 2026-11-13.

## What I did

- The pause's third row is **Garage** (was Switch To Machine). It opens `Screen::PauseGarage`: a page
  of the pausing player's machines as tiles over the dimmed ride (`Menu::pause_tiles`, the machine
  ridden plus `Menu::choices`, so online the room's rules still apply). The cursor starts on the
  ridden tile. Cross switches; on the ridden tile it resumes. L1/R1 step a flier's skin with the old
  P315g rules (`own_flier`, which was `switch_flier`). Circle goes back to the pause; Start resumes.
  The legend is Back · L1 R1 Livery · ✕ Ride.
- **Triangle** quick-switches to the last machine ridden (`Menu::last_up`, filled in by `switched`,
  reset at ride start in main.rs). It works only on the pause list and its Garage, and does nothing
  until a second machine has been ridden.
- **LAST MACHINE** widget beside NOW PLAYING, on the plain pause only. The music box is narrowed from
  32 to 28 ems to make room (`menu_player::LAST_BOX`).
- The Garage **Machines tab** is a grid (`menu_tiles.rs`): sections Bikes / Fliers / Quads & cars;
  one tile per disc bike model and class, each with its paints as liveries; pack bikes greyed with a
  lock; one tile per flier kind with its skins; the quad (liveries Off and the files); other kinds
  greyed. The Setup heading with Driving Model and Handling follows. The tile cards are cream with
  the baked picture, the red pill when lit, and a RIDING tag. The side panel shows the turning model
  as before, the name, "L1 ◀ livery ▶ R1" and the liveries as radio rows (greyed ones keep their why
  as the tip). L1/R1 turn the livery; L2/R2 move the Garage's tabs (both the tab chips and the hints
  say so).
- Tile pictures are baked in the front end by a second `garage_view::Stage` (`Stage::bake`, a soft
  draw with a clear background), one at a time, into `Art::icons`. The GPU gets them in batches of 8
  and once more when the last is done.
- Hint tab: the same `HINT_MARGIN` (2 ems) is left from the slant's middle and from the right edge, and
  the size-down room is worked out with the same numbers, so it cannot clip.
- Removed: the flier picker popup over the pause, and the Machines tab's old Bike Class / Bike /
  Paint / Plane / Helicopter / Quad rows. The tests that walked them now walk the tiles.

## What the original does

Not a port: the original's pause has a two-way Switch To Machine and no garage page. Nothing was decompiled.

## How it was proved

- tests/menu.rs `pause_garage_switches_the_machine_and_triangle_swaps_the_last_two` (new). Also the
  rewritten `picking_the_biplane_in_the_pause_puts_that_player_in_it`,
  `the_pause_switch_picks_the_plane_skin`, `l1_and_r1_turn_the_livery_and_l2_r2_move_the_garage_tabs`,
  `the_fliers_tab_picks_a_renamed_packs_skins_per_seat`,
  `the_machine_picker_lists_pack_bikes_and_round_trips_garage_txt` and
  `the_quad_row_picks_a_handling_file_and_the_pause_offers_it`, plus tests/online_car.rs
  `the_online_pause_offers_the_cars` and tests/machines_everywhere.rs. `press_every_row` (the
  every-row-changes walk) walks a page with tiles with `walk_grid_to` (Left/Right along tiles,
  Up/Down otherwise) and turns a tile with R1. The layout tests that walk
  `Screen::ALL` now cover PauseGarage too.

## Not verified / not 1:1

- **Not looked at on screen.** No shot of the grid, the pause Garage or the LAST MACHINE widget was
  taken, so card sizes, picture framing, lock and tag placement are unchecked by eye.
- The pause Garage shows each tile's baked picture, not the turning model, because
  `Stage::draw_gpu` forgets the ride's assets and so can't run mid-ride.
- Pictures are baked with seat 0's rider and keyed by bike archive only, so another seat's tile shows
  1P's rider. Quad and car tiles have no picture: they show a placeholder letter (the class's first
  letter). Pictures not baked when the ride starts stay placeholders for that ride: `images()` is
  taken at ride start.
- A `--shot` of the Garage may catch tiles before their pictures are baked: baking is not settled for shots.
- Cars are not in the main Garage's grid: the worlds provide them. Pack bikes are locked tiles.
- L1/R1 on a bike tile that isn't ridden picks that model at the stepped paint (a bike's livery is a pick).
- Triangle no longer resumes from the pause: it is now the quick-switch there. Circle and Start still resume.
- The P318a note that greyed your own pick on the pause is gone; greyed skins show their why in the liveries list.
- Mouse/touch clicks on the pause Garage's tiles are not wired: pad and keys only.
- Online, the pause Garage's skins follow the old `own_flier` rules unchanged; no online run was made.

## Next

Take a screenshot of the Garage grid and the pause (`--menu` on a private weston, see memory) and tune card sizes by eye.
