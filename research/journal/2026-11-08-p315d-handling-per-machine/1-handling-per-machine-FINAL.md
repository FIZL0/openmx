# 1 — handling per machine (FINAL)

Task P315d. 2026-11-08.

## What I did

- Moved the handling choice off Settings > Ride. Its Handling and Handling From rows are gone. The Garage's
  Bike tab has a **Handling** row per seat, under Driving Model. It offers Original, then MX vs. ATV and
  Untamed for each installed trick pack whose `mod.json` lists bike `.phy` files (`TrickPack::phy`,
  `trick_packs::Pack::handles`). The pack's name goes on the label only when more than one pack offers
  handling. The row's note says what the pick rides. The row is greyed, with the reason, when mods are off
  or no pack offers anything but the original's.
- The pick is part of `roster::Pick` (`handling: Vec<HandlingPick>`, keyed by machine) and is kept in
  `garage.txt` as `seat<n>.handling = bike <handling> [<pack>]`. The original's writes no line. An empty
  pack reads the first trick pack on, or else the first in the folder, as Handling From's "First Pack On"
  did. A pick naming a pack that is gone falls back the same way, and the ride says so.
- Migration: an older `settings.txt` `handling` / `handling_pack` is read into `Settings::old_handling`.
  `Menu::take_old_handling` (run by `with_garage`) moves it onto 1P's bike pick, unless 1P already has a
  pick. It is never written back. This is the same path P315 took for the Fliers row.
- Rides: `seat_handling` in main.rs gives each seat its own handling and pack. `--handling` /
  `--handling-pack` on the command line still win over every seat. Each seat builds its own phy pack. A
  seat with the same handling as P1 reuses P1's pack. When any seat is off the original's, the ride prints
  `handling: P<n> on <the original's | MX vs. ATV's from X | Untamed's from X>` for every seat. Untamed's
  line is said per seat (`player 2: handling: on Untamed's steps`).
- Brake Gyro and Gas Gyro stay in Settings > Ride as global settings. They are live when any seat rides
  MX vs. ATV or Untamed handling.
- Lobby hosting names 1P's handling (`lobby::name_handling` takes 1P's pick).

## What the original does

Not applicable. This is openmx's own menu and settings; MX Unleashed has no handling choice.

## How it was proved

- `tests/menu.rs` `the_bike_tab_picks_each_seats_handling`: the Bike tab's row steps per seat, labels
  name the pack only when two packs offer handling, a pack without phy files offers nothing, the row is
  greyed with no packs, and the old setting migrates onto 1P (and leaves a seat with a pick alone).
- The every-row walks (`every_row_of_every_settings_tab_and_page_changes_by_pad` and the Garage tabs in
  `a_picked_bike_and_rider_reach_the_ride_that_starts_per_seat`) reach the new row and the gyro rows.
  Their setups now give the menu a trick pack, and give 1P a non-original pick for the gyros.
- `tests/seat_handling.rs`: headless `--garage` rides on free09 with 3, 2 and 1 players. They check the
  `handling: P<n>` lines (MX vs. ATV for P1, Untamed for P2 with no pack named, the original's for P3),
  that `--handling original` overrides every pick, the gone-pack fallback, and that there are no lines
  with no picks. The four rides ran here (disc and `mods/tricks/mxvatv_tricks` present).
- `roster.rs` unit test: `garage.txt` round trip and what the packs offer.
- `tests/lobby.rs` updated to 1P's pick.

## Not verified / not 1:1

- Fliers (plane, helicopter) get no Handling row. A flier flies whatever its skin's pack gives it (P315b),
  so there is no choice to make. If "each machine's row" meant a handling per flier too, that is not done.
- Online and lobby rides still use one handling for the whole room, from the host's 1P pick. Per-seat
  handling online is P318's.
- The Gyro rows are still global settings, not per seat.
- Untamed is offered on every trick pack that has bike files, not only on an Untamed pack. This is the
  same as the old Settings row, which allowed any pack with either handling.
- `--handling-pack` with no `--handling` overrides only the pack of seats whose pick is non-original.
- No screenshot of the new row was taken. It was checked by menu tests only.
- Only bit-for-bit menu and ride-line behaviour is tested. Whether a two-player ride with different
  handlings *feels* right per seat was not watched on screen.
- Nothing was taken from opentennis.

## Next

Nothing for this task. Per-seat handling online is P318's.
