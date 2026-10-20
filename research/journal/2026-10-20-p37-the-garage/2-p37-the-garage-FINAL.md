# 2 — the garage screen, and every seat on its pick (FINAL)

Task P37. 2026-10-20. Continues `1-the-garage-PART.md`, which built `roster.rs` and blocked on P36. P36
merged (2120b2b), so this session put the screen in P36's front end and the picks into the ride.

## What I did

- **`menu.rs`**: `Screen::Garage` ("garage" for `--screen`). It is the main menu's second row, so the menu
  is now Free Ride, Garage, Settings, Quit.
  - `Menu` carries the `Catalogue`, the `Picks` and one `roster::Garage` per seat while the screen is open
    (`with_garage`). `Menu::new` is unchanged, and a menu with no catalogue still walks every screen.
  - Seat 1's pad goes through `step` as before. Every other seat's pad goes through `step_seat(i, press)`,
    which only the garage hears, and only for seats below `seated`. Up and down choose a row, left and right
    turn it. Cross, Circle or Start on seat 1 leave the screen and keep every seat's pick
    (`take_picks_changed`). Another seat's Cross does nothing.
  - `ride_picks(players)`: the first `players` seats' picks, each fitted to the disc, or `None` without a
    catalogue (then the command line rules, as before).
  - Drawing: one column per seated player in the main menu's column texture and red bar. Each of the seven
    rows is a label over its value, with arrows on the chosen row, plus a "Player N" header.
- **`main.rs`**:
  - The front end reads the catalogue with the menu art and `garage.txt` beside `settings.txt`. It keeps
    one `Reader` per seat, sets `seated` to the last seat that has someone in it, and saves `garage.txt`
    when the screen closes with a change.
  - Every ride the front end starts goes through `apply_picks`: seat 1's pick becomes `args.bike`,
    `args.rider` and the new `args.set`, and seats 2 and up go to `args.others`.
  - `ride()`: seat 1's `.phy` is `roster::physics(bike archive, args.set or FREERIDE)`. Each other seat
    opens its own bike archive and reads its own set's `.phy` into its own `Sim::with_rider`; `new_rider`
    now takes the `Phy`. It loads its own rider archive for its body, and its own bike's `.sub`, chassis
    and parts. Those are drawn from models loaded again under `name@N` in that seat's paint
    (`Rig::parts` replaces the draw loop's shared `vehicle.parts`). A seat with no pick rides seat 1's
    things as before.
  - New flags: `--set WORD` (one of the sixteen, like `pro250p`) and `--garage` (ride `garage.txt`'s
    picks without the front end, every seat its own). Both count as picking a ride. `--phy` still
    overrides seat 1's file.
- **`game-render/src/world.rs`**: `Assets::load_repainted(archives, name, own, renames)`, a new function.
  The first try at per-seat gear drew both riders in seat 1's gear, because `texture_of` caches a
  material's answer under the name the mesh asks for (`helmet`), so the second rider's rename never got
  looked at. The new function loads with a fresh cache and the seat's renames, stores the model as `own`,
  then restores the first load's cache, renames and model.
- **`.gitignore`**: `/garage.txt`, beside `/settings.txt` and `/controls.txt`.
- **`tests/menu.rs`**: `a_picked_bike_and_rider_reach_the_ride_that_starts_per_seat`. On the disc, two
  seats on two pads walk their own columns: seat 1 goes to 250 Pro P and the next rider, seat 2 to 50 Race
  A and another rider in its second colours. An empty third seat's presses move nothing, and seat 2's
  Cross does not close the screen. Seat 1 leaves, and the picks are kept and round-trip through
  `garage.txt`. Free Ride then Cross gives `Act::Ride("free06")`, and `ride_picks(2)` is exactly those two
  picks. Each resolves through `roster::physics` to its own `bike\<set>.phy` (no fallback), read by
  `Phy::read_bytes`. Each rider archive loads with its own gear, and the two seats differ in both.
  `what_the_pad_sets_is_what_the_file_says` takes one more Down to reach Settings.

## How it was proved

- `./kit check`: passed (792 tests). The new disc test was forced to fail once with a probe so I could
  see that it ran and did not skip. It printed `seat 1: 250sx on bike\pro250p.phy, tld1` and
  `seat 2: fast50_01 on bike\race50a.phy, tortelli2`.
- The shell itself: `--headless --garage --players 2` on a hand-written `garage.txt` (250sx/pro250p/tld1,
  fast50_01/race50a/tortelli2). It printed `player 2: fast50_01 on bike\race50a.phy, tortelli2` and drew
  two different riders in their own gear on their own bikes. Seat 2's 50 looks the same as the shell's
  drawing of `--bike fast50_01` as seat 1, which also loads 127 of 133 models for it.
- `--screen garage --shot`: the screen draws (one column, since no pad was plugged in).

## Not verified / not 1:1

- **Starting a ride from the front end in a window was not driven.** This desktop delivers no keystrokes
  and the pad is root-only, so Garage → Free Ride → Cross was not pressed in the real window. That path is
  `ride_picks` (tested) → `apply_picks` (the same three lines `--garage` uses, and that was run) → `ride()`.
- **A second real pad in the garage was not tried.** The per-seat columns are proved with reports in the
  test. `seated` comes from `Seats::frame`, and no second device was plugged in here.
- **Seats 2+ use seat 1's wheel mounts and steering axis.** `mounts` and `steer_axis` come from seat 1's
  chassis nodes. All the bikes load the one `bike.psg` through `bike.mcf`, so they are probably equal, but
  I did not check that per archive.
- **Per-seat textures are loaded again, not shared.** `load_repainted` reloads every texture the
  repainted model uses, so a seat's own bike costs its textures again. That is memory, not behaviour.
- **On the front end, the garage pick wins over `--bike`/`--rider`.** `--menu --bike x` rides the
  garage's bike. `--phy` still wins for seat 1.
- **The original's garage was not compared.** Carried over from part 1: which sets the original offers, its
  display names for liveries and riders, and its rider preview (`UI\Rider\uniform_*.res`, not drawn here).
  The screen's layout is the port's own, in the main menu's art.
- **The biplane is not a garage row** (it waits on Phase 11), and the rider's sex is always `Male`. Both
  are carried over from part 1.
- **The set's class does not follow `--set` onto the bike.** `--set pro500a` on the 125sx finds no such
  file and falls back with a printed note. A garage pick cannot do this, because `fit` keeps the class on
  the bike.
