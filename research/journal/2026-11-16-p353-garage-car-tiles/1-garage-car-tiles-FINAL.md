# 1 — garage car tiles (FINAL)

Task P353. 2026-11-16.

## What I did

- The Garage's Machines tab (`menu_tiles.rs`, `Menu::garage_tiles`) now has a tile for each of the
  disc's cars: Buggy, Monster Truck and Baja. They lead the Quads & cars section, ahead of the quad
  and the greyed pack kinds, in the same order the pause's Garage lists them.
- New `TileOf::Car(Machine)`. The tile is not locked (every seat has each car in a ride), has no
  liveries and no picture (the class letter shows, as on the quad's tile). Its side panel says where
  the car is ridden: "switch to it from the pause's Garage" (`CAR_ABOUT`). R1, L1 and Cross on it
  change nothing.
- The baja is shown too, though the task names only the buggy and the monster truck. It is the
  disc's third car, the pause's Garage already offers it, and leaving it out would make the two
  grids disagree.
- tests/menu.rs: new `the_machines_tab_shows_the_cars`. `every_row_changes` leaves out car tiles,
  since there is nothing on them to change.
- Nothing taken from opentennis: it has no machine grid.

## What the original does

Not a port: the Garage grid is openmx's own front end (P341). Nothing was decompiled.

## How it was proved

- tests/menu.rs `the_machines_tab_shows_the_cars`: the three car tiles are in section 2, unlocked,
  in order, under the Quads & cars heading. The cursor reaches the Monster Truck tile by pad. Its side
  text is the car note. R1, L1 and Cross leave the menu as it was.
- The Garage's every-row walk (`the_garage_*` tests) still passes with the new tiles.

## Not verified / not 1:1

- **Not looked at on screen.** No shot of the grid with the car tiles was taken, so their placement and
  placeholder letter are checked only by the layout tests.
- **No pictures.** Car tiles show the "M" placeholder letter, not a baked picture. The tile baker draws
  bikes and fliers only (P341 left the quad without one for the same reason).
- **Nothing to pick.** Each seat's car livery is fixed by its seat, and a ride always starts on the
  bike. So the tile only says where the car is ridden. A per-seat car livery or a start-on-a-car pick
  would need new settings and shell plumbing, which this task did not ask for.
- **Always shown.** The front end does not know before a ride whether the disc's `machine.res` has
  the car files, so the tiles are shown either way. The retail disc always has them.

## Next

Done.
