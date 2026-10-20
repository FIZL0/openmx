# 1 — machine in the menu (FINAL)

Task P41. 2026-10-20.

## What I did

- **The pause** (`crates/openmx/src/menu.rs`) has a third row, "Switch To Machine: Biplane" (or
  ": Bike" in the plane, ": None" for a player with no plane to take). Cross on it returns
  `Act::Switch(seat)` and closes the pause; the shell (`main.rs`'s ride loop) answers with the same
  swap M runs — factored out of the M key's code into `switch_machine` (camera snapped, HUD notes) —
  and tells the menu with `Menu::switched`. M stays for the keyboard and keeps the menu in step.
- **Per seat.** `Menu::step_seats` takes every rider's press: while the ride runs any player's Start
  opens the pause *for them* (`pause_for(seat)`, 1P's Start winning a tie), an open pause hears only
  that player's pad, and with more than one rider the title says whose ("2P Paused"). The shell
  keeps one `menu::Reader` per seat past 1P for this; Escape is Back for whoever drives
  (`Menu::driver`). The ride fills `menu.riders`, `menu.up` and `menu.switchable` from the seats'
  garages when it starts.
- **The ride setup.** P37's garage screen is not in main (P37 is blocked), so the setup is the Free
  Ride track pick: a panel over the stills lists each rider's machine ("Machine  Bike" with one rider,
  "1P  Bike / 2P  Bike…" with more), Up/Down pick the player, Square ("Machine" in the legend) flips
  their machine. Left/Right still walk the tracks; Up/Down no longer do. The picks persist in
  `settings.txt` as `machines = bike biplane …` (one word a seat, 1P first; an older file without the
  line rides the bike). `front_end` reads 1P's pick into `args.vehicle` (`Kind::parse(word)`, the same
  word `--vehicle` takes) unless `--vehicle biplane` was given.
- From opentennis: nothing to copy here — its pause (`main_menu/pause.rs`) has no per-player owner and
  tennis has no machine switch; the per-seat Start/reader shape is my own.

## What the original does

The original's pause menu has "Switch To Machine: %s" (its string is a research note in
`research/oracle.txt`, freeride_switch_commit), which calls the same swap as the 30 ft prompt — R8 and
P19 read that, and `vehicle.rs`'s `Garage::swap` is it. The swap is unchanged by this task.

## How it was proved

- `crates/openmx/tests/menu.rs`:
  - `picking_the_biplane_in_the_pause_puts_that_player_in_it` — two riders, each with a garage built
    from a two-plane `machine.txt`; by pad reports alone 2P's Start pauses for 2P (1P's Down does not
    move it), Down Down Cross on 2P's pad switches 2P's garage to the biplane and leaves 1P's on the
    bike, then 1P's own pause switches 1P; a player with no plane gets ": None" and no switch.
  - `the_setups_pick_starts_the_ride_on_it` — Square/Up/Down on the setup set 1P and 2P's machines,
    Cross rides, 1P's pick through `Kind::parse` builds a garage that starts in the biplane, and the
    file round-trips the picks.
  - The existing pause walk and settings round trip, updated for the new row and field.
- `./kit check`: passed.
- By eye: `--menu --players 2 --screen tracks|pause --shot` (machine panel, "1P Paused", the switch
  row) and `--menu --vehicle biplane --screen pause` (the ride starts in the plane, "vehicle:
  biplane", and the row offers "Bike").

## Not verified / not 1:1

- **The shell's glue was not driven live through the pause.** No keystrokes reach the window here and
  the front end needs a window, so the pause's switch row and another seat's Start were exercised only
  through `Menu::step_seats` + `Garage::swap` in the test (which mirrors main.rs's loop), not by
  pressing them in a running shell. The setup pick's path into `ride()` (`settings.machines[0]` →
  `args.vehicle`) was not run either: that would mean writing the user's `settings.txt` beside the
  disc. `--vehicle biplane` through the same `Garage::new` was run.
- **Only 1P can be in a plane in the shell.** The track's `machine.txt` parks `[Biplane1]` only on the
  tracks read so far, and the shell loads, draws and cameras one plane (seat 0's). So a later seat's
  pause row reads ": None", and a later seat's setup pick of the biplane rides the bike and says so on
  the console. The menu side is per seat and tested with a second plane; the shell side is not.
- **What %s is in the original's "Switch To Machine: %s"** — I show the machine a switch goes to; it
  may be the machine's own name or the one being left. Not read (the menu is ours to design, AGENT.md).
- **Whether the original's pause closes after the switch.** Here it does (the ride carries on in the
  new machine). Not checked against the original.
- **`--vehicle bike` does not override a setup pick of the biplane** under `--menu`; only a biplane
  flag is told apart from the default, because `Args` has no "was it given" for it.
- **Pill widths** of the longer pause row are P39's (the pills are not yet sized to labels).

## Next

none
