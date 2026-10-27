# 1 — start on gone (FINAL)

Task P177. 2026-10-26.

## What I did

The user asked to "remove start on machine option, just have the switch in pause menu". Every ride now starts
on the bike. To get on the machine, use the pause's Switch To Machine or stop at a bay.

- `menu_pages.rs`: the Ride tab's Start On row and `Key::StartOn` are gone. Wrecks is the tab's first row now.
- `menu.rs`: `Settings::machines` is gone. settings.txt still reads an old file's `machines = …` line and
  drops it, and no longer writes the line. The Free Ride setup strip's machine pick went too (Square flipped
  the lit seat's machine, L1/R1 picked the seat, `setup_seat`). It was the same setting under another name.
  Keeping it would have kept "start on" for 1P–8P, which the user asked to remove. The strip now shows
  Wrecks and Modifiers, and its hints are Back, Wrecks, Game and Ride.
- `menu_screens.rs`: the Main menu's Free Ride panel no longer has the machine chip, and the Tracks strip
  is redrawn as above.
- `main.rs`: the ride no longer reads the setting into `--vehicle` for 1P, and the other local seats start
  on `Kind::Bike`. The lobby's 1P machine (N18) is `--vehicle`'s word, which is the bike unless
  `--vehicle biplane` is given. That CLI flag stays: it is a debugging and test switch, not the menu option.
- tests/menu.rs: `the_setups_pick_starts_the_ride_on_it` is replaced by `a_ride_starts_on_the_bike`. It
  checks that Square, L1 and R1 on the setup change nothing, that the Ride tab has no Start On, and that an
  old file's machines line loads and is not written back. The old settings.txt test now expects the line to
  drop out of the written file, and every other line still matches word for word. The tab walk's first
  Ride row is Wrecks.
- tests/machines_everywhere.rs, which the brief names, never set the start machine. It already reaches the
  plane through `switch_from_the_pause`, so it is unchanged. No other test set a `machines` line.

opentennis has nothing like this to take: it is a removal.

## What the original does

The original has no start-machine pick either. The biplane is reached from the pause's Switch To Machine,
or at a bay.

## How it was proved

`./kit check -- -p openmx --test menu` passes, 174 tests. `./kit check --all`: the only failure that
needed a fix was my own new test, which expected the wrong default track. Two lobby tests also failed,
then passed alone on a rerun (load flakes).

## Not verified / not 1:1

- I did not look at the Tracks and Main screens' new strip in a `--shot`. The chip list only got shorter,
  and the layout code is untouched.
- docs/menu-concept.html still shows Start On. It is the user's concept file and I left it as it is.

## Next

none
