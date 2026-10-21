# 1 — ghost collision (FINAL)

Task S63. 2026-10-21.

## What I did

- `crates/game-sim/src/ghost.rs` (new): P57's `Ghost` moved here from `crates/openmx/src/vehicle.rs`,
  bit for bit, with `step(dt, inside)` — `inside` is the overlap query, asked only while the clock
  runs, and the clock does not count on a tick it says yes. `overlaps(me, others)` is that query on
  S54's `Rival` spheres (strictly closer than the two radii, squared through the accumulator). A
  `hash::State` impl.
- `crates/game-sim/src/rider.rs`: `Rider::ghost`. `frame_among` ticks it first (the head of the bike's
  update), its query against the rivals it was handed; both put-backs (the chord's and the wreck's
  own reset) start it, where `Frame::respawned` is set; the head ray is swept against the world alone
  while it is on.
- `crates/game-sim/src/lib.rs`: `step_all` leaves ghosts out of every rider's rival list.
- `hash.rs`: the walk names every player's ghost under `ghosts`, only when there are other riders
  (the one place it reaches the simulation), so every one-player pinned hash stays. `snapshot.rs`:
  the ghost is on the wire, after `airborne`.
- `crates/openmx/src/vehicle.rs`: `Ghost` is a re-export; `Garage` lost its own ghost and its
  tick/start, since `sim.step` now does both (and still is not called with the plane up).
  `main.rs` draws at `seat.sim.rider.ghost.shown()`.
- Pins moved: `tests/hash.rs` size 9472 → 9480; `tests/platforms.rs` `RIDERS_*` regenerated and
  spliced (wire 11895 → 11931 bytes, 9 a rider). Checked first that the riders' run did not *behave*
  differently: with the `ghosts` group taken out of the walk, the old `RIDERS_HASHES` and
  `RIDERS_DIGEST` matched unchanged and only the wire bytes moved — so the regenerated numbers are
  the new leaves, not a new ride. Not re-run on the other architectures (see below).
- `research/oracle.txt`: an S63 block under P57's ghost lines (comments only).
- opentennis: no ghosts or pass-through collision there; nothing taken.

## What the original does

`./kit fn 0x00250580` (ghost_start) adds the vehicle class mask to both of the rider's bodies' ignore
lists (`./kit fn 0x0011b080`), and `./kit fn 0x00250950` (ghost_step) removes it (`0x0011b160`) on the
tick the clock runs out. The pair filter `./kit fn 0x00119d70` rejects a pair when either body's list,
in exclude mode (+0xe8 = 1), holds the other's class; the bike's collide `./kit fn 0x002708a0` runs
its body through the query `./kit fn 0x0011a150`, which filters every body candidate that way, before
it writes the ray contacts S54 ported. So a ghost neither strikes nor is struck.

ghost_step, while the clock is above zero: sets the body's list to include mode, runs the overlap
query, sets it back to exclude; with the list in include mode only vehicles pass, so the query's
"overlapping" bit (+0x116 0x20) says "inside another vehicle", and the clock counts dt only when it is
clear. The other body's list is still checked in its own (exclude) mode, so another ghost rejects
the query: two ghosts inside each other both count down. Consequence the tests pin: a ghost cannot
run out while inside another rider; held at the full clock its alpha is the fade's floor, 0.1.

## How it was proved

Through `./kit check` (passes). New, in `crates/game-sim/tests/ride.rs`:

- `a_ghost_passes_through_another_rider`: S54's two-rider stack jousts; with either rider a ghost it
  does not; a ghost with half a tick left inside the rider above is still a ghost after the tick.
- `a_ghost_holds_its_clock_while_inside_another_rider`: side by side, 45 ticks at exactly 3.0 and
  alpha 0.1; moved 50 away, the first tick counts dt and the ghost ends on the 91st tick, P57's
  count; two bodies just apart do not hold; two ghosts in one place both count.
- `the_ghosts_overlap_query_is_strict`.
- P57's two openmx tests kept, on the new home (`step(dt, || false)`, `sim.rider.ghost`): the fade
  is still bit-equal to the live capture, and L1+R1 through the garage still starts it.

None of this is against the original running: no two-rider session or capture exists (R9).

## Not verified / not 1:1

- **No live proof.** Everything about the collision half comes from the decompilation: there is no
  two-player save state or capture to watch a ghost pass through a rider in (R9 / plan/HUMAN.md).
- **The body shape is the port's own**, S54's sphere per rider round the body rays' hub; the original
  overlaps the bikes' model collidables. When "inside" starts and ends differs near the edges, so the
  hold's length differs too. Touching spheres do not hold (the port's choice for the boundary).
- **One body per rider.** The original's ghost has two bodies (+0x7c4, +0x7cc) on the ignore list and
  queries only the first; the port's rider has one shape, used for both striking and the query. Which
  of the other rider's bodies carry the vehicle class was not read.
- **A downed rider is still nobody's rival** (S54's rule), so a ghost inside a wrecked rider counts
  down; the original would test whatever collidables the wreck leaves up. Not traced.
- **Where bit 0x20 is cleared between ticks** was not found; the port asks the overlap fresh each tick.
  The query's unfiltered type-3 candidates also set it, which the port ignores (P57's live run counted
  down on the ground, so they do not reach a riding bike there).
- **The physical push** between bikes is still not ported (S54), so "passing through" is only the
  head ray and the hold; there is no resolve for the ghost to switch off.
- **The ignore list's other uses** (props, the ragdoll's bike ignore) are not touched; only the
  vehicle class is modelled, as a flag.
- **`RIDERS_*` on other architectures**: regenerated on x86_64 only; the cross script was not run.
  The change is new leaves of plain state, no new float operation.

## Next

none — when R9 brings a two-rider session, watch a ghost pass through and hold (rider +0x7dc) live.
