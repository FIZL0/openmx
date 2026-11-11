# 1 — rider switches and gather (FINAL)

Task S289. 2026-11-10.

## What I did

- Ported the two on/off virtuals that the rider's wreck arm, riding reset and wreck entry call:
  `rider::part_switch` (stores the part's `+4`) and `rider::force_switch` (stores the aim force's `+0`).
  `rider::switch_on` answers `Call::Switch`, picking the force by the rider's `+0x5bc`.
- The nearby gather (`Call::Gather`) is served by F46b's `gather_in_ram`. It reads state 4's broad phase
  out of RAM, runs `atv_world::gather` and writes the result back. It used to live inside
  `tests/mxvatv_bike_ride.rs`. I moved it as it was to `tests/mxvatv_bike_ride/gather_in_ram.rs` (made
  `pub`, with the two vtable constants copied in), so both test binaries can mount it. bike_ride uses it
  through a glob import, so none of its other code changed.
- Two fixes to that shared reader:
  - **Counts read signed.** A collidable's object list and category list counts are now read as signed,
    and a count at or below 0 means an empty list. The original's pair filter (`./kit fn 0x001144f0 -p atv`)
    loops `0 < n`, and the rider test pokes −1 counts into the spheres' ignore lists.
  - **Hash index for `item()`.** `item()` found an entry with a linear search over 15,410 entries, so a
    read was quadratic. It now uses a hash index, rebuilt whenever it stops matching `items` (one caller
    replaces `items` wholesale). One gather went from ~42 ms to ~9 ms.
- `tests/mxvatv_rider_modes.rs`:
  - The original's switch and gather hooks still log each call, then run the hooked function as it is
    (`run_as_is`: a nested `ee.call` below the caller's stack, with the written set, `sp`/`ra` and the
    hooked `pc` put back).
  - The port's `Port`/`ResetPort` answer the calls with `rider::switch_on` and `gather_in_ram`.
  - All switch functions must be one of the two known addresses.
  - Because a gather costs ~9 ms, the mode machine runs it in 1 case in 8, drawn per case (`GATHERS`).
    In the other cases neither side runs it, as before S289.
  - The probe's ignore list sometimes names a collidable the gather finds on the image (found by one
    original gather before the loop), so the pair filter's exclusion is exercised.
- `tests/mxvatv_bike_ride.rs`'s rider leaf now also serves the switches (each vtable slot asserted) and
  the gather in Rust.
- opentennis: nothing to take (no rider ragdoll or broad phase there).

## What the original does

- `0x0049bd30`: `jr ra` with `sw a1, 4(a0)` in the delay slot. `0x0049bd80`: the same with `sw a1, 0(a0)`
  (`./kit dis -p atv 0x0049bd30`). Every hold and named part on state 4 has the first one at slot `+0x10`
  of the vtable at `+0x10`. The aim force has the second at slot `+0x10` of the vtable at `+8`.
- The gather `0x001d6a60(list, probe, 3)` is F46b's `atv_gather_nearby`: rider list `+0x5b8`, the first
  part's sphere as the probe. The list has its filter on (`+0x38` 1) and is flat 0. The spheres' `+0xe0`
  is 1, so they exclude what their lists name.

## How it was proved

- **Function oracle**, `crates/game-sim/tests/mxvatv_rider_modes.rs`, against SLUS-21104's code run in
  the interpreter:
  - Switches: in all three tests, 60,000 mode-machine cases, 20,000 riding-reset cases and 20,000
    wreck-entry cases.
  - Gather: ~3,200 gathers in mode 2. The probe's ignore list named a found collidable 1,287 times, named
    only others 865 times, and was empty or none 1,046 times.
  - Every word written off the stack is equal on both sides, and so is the call log.
- **Replay**, `crates/game-sim/tests/mxvatv_bike_ride.rs`: `ride_replays_with_the_rider_leaf_on_the_ports`
  (1,836 switches) and `flip_replays_with_the_rider_leaf_on_the_ports` (1,501 switches, 68 gathers) stay
  bit-exact, with the same RAM differences as before.

## Not verified / not 1:1

- **The gather runs in 1 mode-machine case in 8, not all 25,000 mode-2 cases**, to keep the test at about
  a minute. Its inputs barely change from case to case: the probe's centre and the broad phase are never
  poked; only the probe's ignore list is.
- **The gather always found candidates.** The probe sits at one place on the state-4 image, so "nothing
  found" was never taken. That gather path is F46b's ground and is proved there.
- **The switches are ported only for state 4's vtables.** `switch_on` tells the force from the parts by the
  rider's `+0x5bc`, as the original's vtables do on state 4. Another part class with another on/off
  virtual would not be caught in the port, only by the tests' vtable asserts.
- **The PC exe was not read for the switches.** Each is one store, so there are no branches to list.

## Next

None: S289 is done.
