# 1 — the garage (PART)

Task P37. 2026-10-20.

## What I did

P37 hangs its screen off P36's front end (`crates/openmx/src/menu.rs`, the shell's `ride()` split, `tests/menu.rs`).
When this session started P36 was still running in slot s1 on `task/P36`: `menu.rs` not committed, and 344
changed lines in `main.rs`. Building the screen on top of that would mean editing the same hunks and getting a
conflicting merge. So this session did the half that does not depend on it, in a new module, and blocks the rest
on P36 (the approach [[after-tasks-may-still-be-running]] describes).

- **`crates/openmx/src/roster.rs`** (new): the garage without a picture.
  - `Set`: the sixteen `.phy` sets, built by `track::physics_file` (D7's loader name) from a class, a tier and a
    tuning. `Set::FREERIDE` is `Bike\Race125P.phy`.
  - `physics(archive, set, model)`: the shell's `.phy` lookup moved out of `main.rs`, with the set as a
    parameter. It looks for the set's own file, then `<model>.phy`, then the first `.phy`, and returns a note
    when it falls back. This is the function a seat's pick goes through to the simulation.
  - `Catalogue::read(disc)`: every `Bike\*.res` (54), each with the class whose four `.phy` names it
    carries, split into model and livery at the first `_` (`cr125_faisst` is the `cr125` in Faisst's
    livery). Also every pickable `Rider\<name><1|2>.res`: 40 names with two colourways each, which is 80.
    The `ff_*` freestyle copies, `ghost_ai_01` (the pack's AI body) and `custom` (the create-a-rider
    wardrobe, which has no `.sub` named after it) are left out.
  - `Catalogue::fit`: keeps a pick on something the disc has. The set's class always follows the bike,
    because a model archive carries only its own class's four files.
  - `Pick`/`Picks`: one pick per seat for `SEATS = 4` seats, stored as `garage.txt`
    (`seat<n> = <bike> <set> <rider>`). Writes are atomic (a `.part` file, then a rename). Parsing is lenient
    in the same way as P36's `settings.txt`.
  - `Garage`: one seat's state machine. Up and down choose a row (class, model, livery, tier, tuning, rider,
    colours), and left and right change that row's value.
- **`main.rs`**: `mod roster;`, and the `None` branch of the `.phy` pick now calls
  `roster::physics(&archives[1], Set::FREERIDE, &args.bike)`. The behaviour is the same: the same three-step
  lookup and the same printed line. Nothing else in `main.rs` changed, so the hunks P36 is editing were left
  alone.
- **`crates/openmx/tests/garage.rs`** (new, not `tests/menu.rs`: P36 creates that file and two branches
  adding it would collide). It compiles `roster.rs` in by path, as `tests/ride.rs` does with the pad. Tests:
  - `a_pick_without_the_disc`: the file parses leniently and the text round-trips.
  - `the_disc_offers_every_set_and_the_rider_set`: 54 bikes, all four classes have models, 80 riders under
    40 names, the stock pick is valid, and a garage driven by `turn` alone reaches all sixteen sets. For each
    one, the member that answers is the set's own file.
  - `a_picked_bike_and_rider_reach_the_ride_per_seat`: four different seat picks are saved and loaded back
    unchanged. Each one resolves to its own `.phy` member, read by `Phy::read_bytes`, and each pair of seats
    differs. Each seat's rider archive loads through `game_render::rider::Rider::load` with that seat's
    gear, again different for every pair. The stock seat resolves to `bike\race125p.phy` out of `125sx`.
    Riding Race125P and Race50A flat out for 180 frames on a flat floor gives different speeds: 50.88 and
    90.83 ft/s.
  - Four unit tests in `roster.rs`.

**From opentennis:** the shape of `crates/hst/src/play/main_menu/roster.rs`. Each seat is its own column
with its own cursor and picks, and left/right walks a row's choices (the costume and hand "pills"). What
changed: opentennis's roster is a Bevy UI over a character list. Here it is a pure state machine (`Garage`)
over what the disc lists, and the picture is left to P36's `Art`.

## What the original does

D7 (`research/journal/2026-10-15-d7-d7-freeride-bike-class`) covers the loader. It builds two names,
`Bike\<Race|Pro><cc><P|A>.phy`, from the event's class and tier, and reads them out of the mounted bike
model archive. Each model archive carries its own class's four files (checked here for all 54). The
original's own garage front end, and the career profile that would write the event's class and tier, were
not read in this task. AGENT.md leaves the front end to the port.

## How it was proved

- The disc: `tests/garage.rs` against the user's ISO. 54 of 54 bike archives have a complete class, all 16
  sets are reachable and answered by their own file, and 80 riders under 40 names load.
- The ride: per-seat `.phy` bytes go through the ported loader and the ported simulation (`Sim::with_rider`,
  `poll`, `step`), and two picks ride differently.
- `./kit check`: passed (684 tests).

## Not verified / not 1:1

- **No garage screen, and the shell does not read `garage.txt` yet.** The rows, the art and starting a ride
  from a seat's pick all depend on P36's `menu.rs` and its `ride()`, which were not merged. Today a pick
  reaches the simulation only through `roster::physics` and the tests, not through the running shell. The
  shell still rides `--bike`, `--rider` and `--phy`, defaulting to the stock pick. This is why the task is
  blocked.
- **"Per seat" covers four seats in the file, but the ride is single-player.** The shell rides one bike.
  Putting seats 2–4 on their own bikes is S54/P35's multi-player ride, which is still running in other slots.
  The test proves each seat's pick resolves to its own files, not that four bikes ride at once.
- **The original's garage was not compared.** I did not trace which class, tier or tuning the original's
  bike select writes for a given model, or whether it offers Pro and A/P to the player at all, so offering
  all sixteen sets is the port's design. D7's open end also remains: which model archive answers the
  loader's request in the original is not chased. The port reads the picked model's archive, which carries
  the file under that exact name.
- **A and P are both offered, but only one is ridden.** The original's loader reads both files of a pair
  (P first). The port rides the one file the pick names, as the shell already did with Race125P. What the
  second file of the pair is for in the original is not known.
- **"Rides differently" is shown on stock geometry.** `flat_out` uses `game_sim::stock`'s wheel mounts and
  a flat floor, not the picked model's `bike.psg` mounts, so the two speeds are not the original's. They only
  show that the picked `.phy` changes the ride. The 50 being faster than the 125 here was not investigated.
- **The rider's sex is always `Male`.** No rider archive on the disc has an `frider` motion set, so there is
  nothing to choose. Not proved for `custom`, which is excluded.
- **The biplane is not a garage row.** It is the track's machine behind P19's switch. The brief says "once
  Phase 11 has it", and that select does not exist yet.
- **Livery names are the archive's suffix** (`faisst`, `rs_01`, `thq_01`), not the original's display
  names. Neither the original's names nor the `UI\Rider\uniform_*.res` preview art were read.
- **`SEATS = 4`** stands for P38's four on one screen. The eight-seat stretch is not covered.

## Next

After P36 lands (and P35 for seats): add a `Screen::Garage` to P36's `menu.rs`. Make it one column per
joined seat, each driving its own `roster::Garage` from that seat's `Press` and drawn with P36's `Art`, with
`UI\Rider\uniform_*.res` as the rider preview. Save `Picks` beside the disc on leaving, next to
`settings.txt`. In P36's `ride()`, open `pick.members()` for seat 1 (and every seat once S54 rides more
than one) and pass `pick.set` to `roster::physics` in place of `Set::FREERIDE`. Then move or extend the
assertions in `tests/garage.rs` into `tests/menu.rs`'s "every screen by pad alone" walk.
