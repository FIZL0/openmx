# 1 — online put back ghost (FINAL)

Task P59. 2026-10-21.

## What I did

- Chose to **carry the ghost in the rollback state** over deriving it from a respawn tick. The `Sim`
  keeps no "put back at tick" field, so a derived ghost would still need a per-seat number in the
  saved state, and a resync/restart would lose it; carrying the ghost itself is the same amount of
  state and needs nothing replayed.
- `net/rides.rs`: `Rides::seat` is one seat's share of a tick (poll, step, `after`) and returns
  what the rider said, so a wrapper can see `respawned`. `Rides::step` now calls it once a seat,
  in the same order, so the steps are unchanged.
- `online.rs`: `Seats { rides, ghosts }` is the `Game` the shell's online ride runs.
  `online::State = (rides::State, Arc<Vec<Ghost>>)`. Each seat's tick works as `Garage::step`
  does for the bike: the ghost's clock first, then the seat's ride, then `start()` again on a
  put-back. Save/restore copy the ghosts. The hash folds every ghost (clock bits, alpha bits,
  flag) into the ride's hash, so a ghost that differs is caught as a desync. In the encoded
  bytes, 9 bytes a seat come after `Rides`' own; decode checks the flag and the length before
  it writes anything. `Online` keeps `ghosts`, and `Online::ghost(d)` returns shell seat `d`'s.
- `main.rs`: after `on.show(d, …)` the shell copies `on.ghost(d)` into `seat.garage.ghost`, which is
  what P57's draw already reads for the bike, its parts and the rider. Online, `Garage::step` is
  never called, so nothing else writes it.
- `tests/lobby.rs` now mounts `vehicle.rs`, because `online.rs` uses it.
- Nothing from opentennis: its rollback has no per-player presentation state like this one. The
  `Seats` wrapper has the same save/restore/hash/encode shape as `Rides`.

## What the original does

Nothing new. P57's ghost (`vehicle::Ghost`, journal `p57`) is the original's, read live. The
original has no online play, so "every peer draws the same fade" is the port's own requirement.

## How it was proved

- `crates/openmx/tests/netplay.rs::a_put_back_on_one_peer_is_a_ghost_on_both`: two machines
  (`host,g`) for 420 rounds, on the harness's link with 20 % loss and 3 rounds of lag, so ticks are
  guessed and run again (the test asserts `reruns > 0` on both). The host's seat presses L1+R1 for
  one tick every 120. Every settled tick's hash agrees on both machines, and that hash has both
  seats' ghosts in it. The settled ticks are then ridden again offline through `Seats` on the
  logged pads: the offline hashes match the online ones tick for tick. The put-backs land 120
  ticks apart. The guest's bike is never a ghost. The bikes match a ghost-less `Rides` on the same
  pads, so the ghost moves nothing. From the first put-back, the host's ghost equals a bare
  `Ghost::start` then `step(tick_dt)` sequence on every tick: half on the put-back's tick, 92
  states to solid.
- `…::the_ghost_goes_over_in_the_rides_bytes`: an encode taken mid-fade and decoded on a fresh ride
  gives the same ghosts and hash, and the two ride on with equal ghosts for 100 ticks. A bad flag,
  a cut-short state or a seat-count-only state is refused, and nothing is written.
- `./kit check` passes overall.

## Not verified / not 1:1

- **Not watched in a real two-window online ride.** The shell's draw path is P57's, now fed from
  `Online::ghost`. I did not run two shells and look at the picture: the proof is headless, on the
  session and the state the draw reads.
- **The online state hash changed.** It now includes the ghosts, so an online shell from before P59
  and one from after would desync against each other. Both sides of a ride run the same build
  anyway.
- `peer.rs` (N4's browser/desktop test ride) still runs plain `Rides` with no ghost. It draws no
  bikes, so there is nothing to show yet.
- The original's ghost also switches off rider-vs-rider collision and freezes its clock while it
  overlaps another vehicle. The online seats do not collide with each other (S61), so neither is
  ported, the same as P57.

## Next

Nothing.
