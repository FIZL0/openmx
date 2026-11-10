# 1 — race menu (FINAL)

Task P327. 2026-11-09.

## What I did

- **Menu.** The main menu has **Race** as row 1, beside Free Ride; Local Play and the rows after it move down one. The new screens are in `crates/openmx/src/menu_race.rs`, which `menu.rs` mounts as `menu::race`:
  - **Race:** the track pick. It lists Supercross 01–24 by archive, and Left/Right/L1/R1 switch to the 22 Nationals, listed by the disc's names.
  - **Race Setup:** Laps 1–20 and Riders 1–8, then Start Race.
  - **Results:** drawn from `menu::Results`, a stand-in shaped like a race's results. It shows dashes for every number the sim lacks.
- **Starting a race.** Start Race sets `Menu::racing()` and returns `Act::Ride(track)`.
  - A cancelled load goes back to the setup. A failed load drops the race.
  - When the ride is left, the shell calls `race_left()`, which opens Results over the main menu.
  - A Free Ride clears `racing`.
- **The race's start** (`crates/openmx/src/race_start.rs`) uses only what `game_sim::race` has:
  - The drop-gate prop is the scene's first placement whose name contains `dropgate`. Its across is up × look, and its half width and height come from its model's box. `Supercross::place_drop_gate` places the gate from it.
  - The gate-drop times are the `Sound*GateDropTime` keys under `[RaceStart<class>]` in the track's second res's `aud\event.aud`.
  - Then: `reset` with the intro counted as seen, `shuffle_lanes` for the riders count, and `grid_spot` for card 0.
  - The shell snaps the grid spot to the ground and faces the bike through the gate. It then sets `Sim.race` to `Race::new(gate, [rider], delay, rng)`, so the existing `Sim::race_frame` holds the rider at the closed gate and releases them on the drop.
  - `--race --laps N --racers N` start the same ride from the command line. The shell prints `race: … closed; … grid …; drop in …s` and `race: gate dropped at tick N, rider at …`.
- **HUD.** A pill at the top centre reads `LAP -/N  POS -/M` (`ride_hud::RaceShown`).
- **Unchanged.** Nothing in `crates/game-sim` changed. Free Ride never builds a `Race`, and its start (`no_start_line`, spot, heading) is untouched.
- **Existing tests.** In `tests/menu.rs`, the local-play tests that walked down one row from Free Ride now walk two. The reachability walk leaves out Results, which only the shell opens after a race. `tests/lobby.rs`' `to_online` walks three rows down instead of two.

## What the original does

The front end and race flow were not decompiled for this task. The gate, grid and drop are S13a/S13b/S13e's ports in `game_sim::race`, used here as they are.

## How it was proved

- `tests/race_menu.rs` runs the binary headless on sx01 with `--race --racers 4`:
  - The gate sits at S196's image centre (101.49, 87.87, 454.58), closed.
  - The grid is behind it at z 446.6, against the gate's 454.6.
  - The gate drops at tick 97, matching its 3.17 s countdown.
  - At the drop the rider is still behind the gate.
  - A free sx01 ride prints no `race:` line.
- `race_start.rs` unit tests:
  - The bike's class picks its start calls.
  - sx01's prop puts the gate at the image's centre, with across +x and a delay of 1.5 + 1.54.
- `tests/menu.rs`:
  - `race_from_the_main_menu_rides_its_setup_and_shows_results` is the pad walk from Main → Race → Setup (laps and riders stepped) → Act::Ride("sx01") → results with dashes → Main.
  - `a_free_ride_is_no_race_and_a_cancelled_race_goes_back_to_its_setup` covers what its name says.
  - `the_race_pages_are_shot_from_the_menu` shoots the three pages with `--screen`. Each is its own picture, and the pick and the setup show the cursor's red row.
- I looked at the menu shots and a race ride shot at frame 50 by eye. The HUD pill shows `LAP -/3 POS -/4`.
- Free Ride replay hashes: the existing gate tests (`./kit check --all`).

## Not verified / not 1:1

- **Supercross names.** The tracks are listed as "Supercross NN" by archive, because the disc's stadium names are not yet mapped to the sx files.
- **The prop's bound.** It is the box around the model's triangles (`animated_toy::bound`), not the original's own prop bound (S297 is unported). sx01's agrees with S196's image; the Nationals' gates were not checked against an image.
- **The prop pick.** The drop-gate prop is the first placement whose name contains `dropgate`. nat01 has two such placements, and which one the original uses is unchecked.
- **No intro, no start hold, no start timeline.** These are S290/S304's in game-sim, and the brief forbids changing game-sim. Only the gate's watch holds the rider.
- **The race's rng.** It is its own `Rng::from_seed(0)`, not the original's shared draws, so which lane the human gets is not the original's.
- **One human and no AI riders.** The riders count only spaces the grid and sets the lane shuffle (it is the setup's NumberOfRacers). S12c/S12e add the AI.
- **No laps, places or finish.** The HUD and the results show dashes, and the results are a stand-in until P313 wires in `race::Results`.
- **How a race ends.** Leaving a race by the pause's Main Menu shows the results; there is no finish yet. The original's flow after a race was not checked.
- **The menu layout.** The Race and Setup pages are this port's own design, not the original's race menus.

## Next

None for P327. Follow-ups are filed in the plan.
