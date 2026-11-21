# 1 — parked car put back on restart (PART)

Task S500. 2026-11-21.

## What I did

- `car::Parked` keeps its put-back store (`Store`: spot, nose, up) and its model. After the load the store
  holds the pair squared once, as the original's does.
- `Parked::put_back`: builds the car again at the store's spot, turned by the store's pair squared
  (`Machine::new`), rebuilds its collision object, then squares the store in place.
- `Parked::settled_after(…, restarts)`: the load's build, `restarts` put-backs, then the load's settle.
  `Parked::settled` is that with 0.
- `vehicle::InPlay::settled_after` passes the count through. The shell counts pause-menu Restarts of one
  ride (`Args::restarts`, main.rs's ride loop) and parks the cars with it. The shell's Restart loads the
  ride again, so the count is the only thing carried over.
- `snapshot.rs`: the store and model are not in the wire state (they move only on a Restart, which
  reloads); a taken state copies them from this machine's car, as it does the hull.
- R68 has not landed, so a driven car's moved store is not handled (see below).
- Nothing from `../opentennis`: it has no parked machines.

## What the original does

- R67's chain: the pause menu's Restart runs the world reset; it runs every actor's reset
  (`./kit fn 0x0021aa80` over the actor list); the car's is the put-back `./kit fn 0x00396e90`:
  `./kit fn 0x0015a030` (place on the store, squaring it in place: `./kit fn 0x0015a1b0`), car +0x2c0 = 1,
  each tire's state cleared (`./kit fn 0x0039ace0`) and the engine's (`./kit fn 0x00399e20`).
- The body after the put-back: at the store's spot, turned by the newly squared pair, velocity and rates 0.

## How it was proved

- A save-state RAM image in the interpreter: `crates/game-sim/tests/car_spawn.rs`
  `a_restart_puts_the_parked_baja_back_as_the_original`, on `r52-free07-baja.ee.bin`. The port's load
  store matches the image's (x, y, z bit for bit). Then the original's actor-manager reset of every actor
  is run twice, and `Parked::put_back` twice beside it. After each: store spot, nose, up and the body's
  position, quat, last quat, velocity and rates bit for bit (x, y, z; quat all four).
  - After one: quat 3d4d3378 3f3220f5 3cd26c82 3f374c7b, R67's bits.
  - After two (three squarings): also bit for bit. This closes R67's "repeats past the second".
- The shell: `vehicle.rs` `a_restart_puts_the_parked_truck_back_from_its_store` (free09's truck on a
  flat floor): two Restarts leave the store squared twice more and the spot kept.

## Not verified / not 1:1

- **The whole world reset was not run.** The interpreter stops on a null function pointer elsewhere in it
  (pc 0x8). The test runs only its actor reset. R67 found both gave the same car bits with the CLI oracle.
- **What happens after a Restart's put-back.** The port settles the car with the load's 90 frames (the
  shell's Restart loads the ride again). Whether the original steps it before play again, or drops it
  in play, is not known. No live pause → Restart was recorded: no pad here (uinput is root-only).
- **Store w lanes.** The original's stored nose and up have w = 1.0. The port's have the file nose's 0.
  The turn reads no w (S498 proved the quat).
- **Other state the car's put-back clears** (tire state, engine words, car +0x2c0) is the port's fresh
  `Machine::new`, not compared word by word after a restart. The body words above were.
- **Only the first put-back of a car asleep or rolling in its bay.** A car the Sim has struck and pushed
  is put back from the same store (the store does not move for that), but this was not run.
- **A driven car** (R68 open): the original's store follows a driven car, so a Restart after driving puts
  it back elsewhere. In the port the driven car is the player's own, not the parked copy, so a Restart
  puts the parked copy back at its bay.
- **Pause row 8 and the results' Restart, and the world reset's other callers:** not wired. The port has
  one Restart (the pause menu's `Act::Restart`). The results screen's Restart starts the race as a new
  ride (count 0); whether the original's results Restart is the same world reset was not checked, and
  race tracks may park no machine at all (not checked).
- **Fliers:** left alone, as R67 found their actor reset does. Their other restart paths were not traced.
- **Online:** the shell has no Restart online, so nothing there.

## Next

- Done once `./kit done` lands.
