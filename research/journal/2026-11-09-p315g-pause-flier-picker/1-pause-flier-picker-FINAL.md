# 1 — pause flier picker (FINAL)

Task P315g. 2026-11-09.

## What I did

- **Menu** (`menu.rs`, `menu_screens.rs`): Cross on the pause's Switch To Machine row, when it would
  put the player in the plane or the helicopter, now opens a picker over the pause, drawn like the
  leaving/confirm popup. The picker is the Garage Fliers tab's row for that kind: `roster::flier_choices`
  stepped with `roster::step_flier`, which skips the greyed skins. It shows the kind (`Plane` /
  `Helicopter`), `< skin name >` and a Switch row. Under those it shows the skin's game and about
  line, and the first greyed skin with its why ("and N more"). The hints are Circle Back, L1/R1 Skin
  and Cross Switch.
  - Left/right or L1/R1 step the seat's `picks.0[pauser]` and mark `picks_changed`, the same way the
    Garage keeps a pick.
  - Cross gives `Act::Switch` as before. Circle or Start goes back to the pause, with the skin kept
    and no switch.
  - Only seats 0 and 1 get the picker, and only offline. Online is P318, and seats past two wear the
    first two seats' liveries (`vehicle::livery`). Every other case switches at once, as before.
  - Why a popup and not another pause row: an extra row would move every row index that tests and
    flows use. Why not L1/R1 on the switch row: L1/R1 already drive the pause's music player.
- **Shell** (`main.rs`, new `flier_refit.rs`):
  - The load's per-livery plane files (phy, MX vs. ATV model, rays, hull), the plane lift and the
    helicopter build moved into `flier_refit` (`PlaneFiles::read`, `plane_lift`, `kiowa_ride`). The
    load calls these, and so does the refit.
  - `flight_for` now reads its files from a shared `Arc<Mutex<Vec<PlaneFiles>>>`. That means a
    refit livery is also what every later flight of it is built off, including joiners and
    `lot.plane` for seats that come in later.
  - `Act::Switch` (offline): where the menu's picks for the seat differ from the ride's,
    `repick_fliers` sets them into `args` (`--fliers` is let go of, and each livery is pinned to what
    it flew) and rereads `fliers_for(args, quiet)`. Then `refit_fliers` rebuilds each livery and kind
    whose pack/skin or disc livery changed:
    - Plane: its files, stock file, lot lift, and every seat-of-that-livery's `Plane.flight` (fresh
      from `flight_for`, with current mods through `replane`, and `attitude.flown` kept) and lift.
    - Helicopter: `lot.kiowa`, and every such seat's `garage.heli` (bay, taken_at and wrecks kept,
      then `Garage::modify`).
    - Drawing: the models go into `assets` and `plane_drawn`/`kiowa_drawn`, the software path's
      posed copies are dropped, and the GPU gets the new models.
    - Then the switch runs as before. Picks are saved to the garage file.
  - Each switch into a flier says `skin: P1 biplane Plane (3) (biplane3) from mxvatv_fliers`.
  - **Tapes:** a switch into a flier is recorded as `biplane@mxvatv_fliers/biplane3` (or
    `@original/biplane2`). MX Unleashed's first livery stays a plain `biplane`, so tapes without
    picks read as before. Playback applies the skin to seat 0's ride picks (not the garage), refits,
    then switches.

## What the original does

Nothing to port. MX Unleashed has one plane and one helicopter with fixed liveries, and its pause
Switch To Machine just swaps. This is front-end and mod work (P315's fliers from machine packs).

## How it was proved

- `tests/mxvatv_fliers.rs::the_switch_flies_the_skin_picked_mid_ride`: a ride loaded on MX Unleashed's
  planes plays a hand-written tape. It switches into `biplane@mxvatv_fliers/biplane3`, back to the bike,
  into `biplane@mxvatv_fliers/biplane`, bike, `biplane@original/biplane2`, bike,
  `helicopter@mxvatv_fliers/kiowa4`, and bike. The ride lines are exactly:
  ```
  skin: P1 biplane Plane (3) (biplane3) from mxvatv_fliers (the tape's)
  skin: P1 biplane Plane (1) (biplane) from mxvatv_fliers (the tape's)
  skin: P1 biplane Biplane, Second Livery (biplane2) from MX Unleashed (the tape's)
  skin: P1 kiowa Helicopter (4) (kiowa4) from mxvatv_fliers (the tape's)
  ```
  The same test also checks:
  - The vehicle lines are bike, biplane, … in order. The helicopter climbs 36.9 ft on Cross off the
    pack's file.
  - The plane rebuilt mid-ride flies bit-alike to one loaded on that skin: the `plane hash` of a
    switch into `biplane@mxvatv_fliers/biplane4` off MX Unleashed's equals that of the ride loaded
    with `--fliers mxvatv --flier-skin 3`, and differs from MX Unleashed's.
  - A `--record` off the played tape writes the same switch words back.
- `tests/menu.rs::the_pause_switch_picks_the_plane_skin`:
  - The picker opens on Switch To Machine.
  - Right and R1 step the disc's two planes, and the pick lands in `picks.0[0]` with
    `picks_changed`.
  - Circle backs out with no switch, and the picker reopens on the kept skin.
  - Cross switches.

  The P84 test (`picking_the_biplane_in_the_pause_puts_that_player_in_it`) and machines_everywhere's
  `switch_from_the_pause` now press Cross twice for seats 0 and 1.
- `./kit check --all`: 5178 passed.

## Not verified / not 1:1

- **Not watched on screen.** The picker popup has not been shot: no `--menu` screenshot exists, and
  the layout is guessed to fit the confirm popup's box (the extra lines are at y 29.4 and 30.6 of a
  box ending at 31). The mid-ride model swap has not been seen drawn either. The tape runs headless,
  and the GPU upload path (`upload_models` after the refit) has not been run on a window.
- **No turning-model preview** in the pause picker, unlike the Garage stage. Filed as P326.
- **Pack models named like the disc's are drawn wrong.** `biplane`, `biplane2`, `kiowa`, `kiowa2` and
  the rotor parts draw whichever loaded first. In the tape above, "Plane (1) (biplane)" flies off the
  pack's file but would be drawn with the disc's model already in `assets`. This was already true at
  the load for mixed P1/P2 picks. Filed as P325.
- **Seats 3 and 4** get no picker. They wear seats 1 and 2's liveries, and a refit of a livery
  rebuilds their parked machines too.
- **A machine left parked somewhere** (not up) by a seat of the refit livery gets a fresh flight or
  helicopter. Its next take lands it where the player is, as every switch does. A helicopter left
  standing loses its `flown` body. I have not checked whether that draws it back in its bay rather
  than where it was left.
- **The `--fliers` override is dropped.** A pause pick on a ride started with `--fliers` lets go of
  it, after pinning each livery's picks to what it flew. This is untested beyond the code path.
- **A played skin word** goes into the ride's seat 0 picks only, not the garage file. A tape read by
  a build before P315g fails to parse `biplane@…` switch words.

## Next

Nothing for P315g. P325 and P326 are the follow-ups.
