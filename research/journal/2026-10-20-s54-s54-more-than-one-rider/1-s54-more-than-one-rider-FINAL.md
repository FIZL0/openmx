# 1 — s54 more than one rider (FINAL)

Task S54. 2026-10-20.

## What I did

- `crates/game-sim/src/lib.rs`: `Sim::others: Vec<Player>` holds players 2 to 8 (`Player { phy, rider }`,
  each with their own bike numbers and their own `Rider`: controls, bike, tricks, wreck, recovery).
  Player 1 is still `Sim::rider` on `Sim::phy`, so the shell, the harness and every test that reads
  `sim.rider` are untouched. New: `MAX_PLAYERS` (8), `Sim::add_player`, `Sim::players`,
  `Sim::rider_of`, `Sim::poll_all` (player `i` on pad `i`, neutral controls past the two pads the
  device manager has), and `Sim::step_all(dt, &[Controls], world)`: every rider in player order, each
  whole frame before the next one's, one tick. `Sim::step` with others present goes through
  `step_all` with player 1's controls and the others' own pads, so `Sim::frame`, `tick_in` and
  `replay::verify` step every player. With no others `step` is the old body, line for line.
- `crates/game-sim/src/ride.rs`: `Rival` (what a rider is to another's head ray: a body centre and
  reach, and the chassis height and velocity `air::Other::Rider` carries), `Ride::rival`, and
  `hits_among`: the head ray swept against the world *and* the rivals, nearest kept. With no rivals
  it returns `hits(ride, world)` itself.
- `crates/game-sim/src/rider.rs`: `Rider::frame_among(…, rivals)`; `Rider::frame` is
  `frame_among(…, &[])`. The only change inside is the wreck test's contact list coming from
  `hits_among`.
- `crates/game-sim/src/hash.rs`: the walk adds a `players` list only when there are others, so a
  one-player walk has the same leaves and the same hash. `tests/hash.rs`' size pin moved
  10224 → 10248 (the `Vec`); the leaf count of a fresh state (821) did not.
- `research/oracle.txt`: an S54 block with the step-order and contact readings below.
- `tick.rs` needed nothing: `Game for Sim` goes through `tick_at` → `frame` → `step`, which now steps
  everyone.
- opentennis: its sim holds a fixed `[Player; 4]` (doubles), not a variable field of riders stepped
  in order; nothing taken.

## What the original does

R9 had not landed, so this comes straight from the decompilation.

**Order.** The race/freeride start (`./kit fn 0x00207b20`) makes the humans first, in a loop over the
player index: each player is new'd, initialised with its index, and appended to the player manager
(`./kit fn 0x002459b0`), which also links it into the scene tree through `./kit fn 0x00132f50`. That
link appends the child at the **tail** of the parent's sibling chain. The tree's update walk
(`./kit fn 0x00133500`) visits children first-to-last in sibling order and calls each one's update,
recursively, and the engine tick's simulation is that update on the root. The ridden actor's update
is reached through a thunk off its `+0x50` node (`./kit dis 0x00281950 0x00281958` → `0x0027c890` →
`0x002520d0`), i.e. by this walk. So: riders are stepped in player order, humans first, each one's
whole update before the next one's starts — for two, and the same rule for any number, so there is
nothing to "extend" beyond keeping the order.

**Contact.** `./kit fn 0x002708a0`'s last loop (after the world search) adds one own entry per ray hit
whose `+0x74` is the struck collidable's collision record; when that is another record (another
rider), it raises that record's touched words and adds a **mirrored** entry to it. The wreck test
(S37) reads only own entries, and a struck *rider* is a Joust (reason 7) when their chassis is more
than `JoustWreckHeight` higher, thrown at their chassis velocity — already ported as
`air::hit_wreck`'s `Other::Rider` arm. That arm is now reached.

## How it was proved

Tests in `crates/game-sim/tests/ride.rs`, all through `./kit check` (687 pass):

- `one_rider_hashes_as_before`: 240 ticks of `step_all` with one player against the rider's own frame
  stepped by hand (the pre-S54 `Sim::step`), hash equal every tick; no `players` group in the walk.
  Plus every pinned hash in the tree (platforms.rs, replay, the ride tests) still passes unchanged.
- `two_riders_step_in_player_order`: 90 ticks of a two-rider `step_all` against player 1's
  `frame_among` (player 2 as it stood) then player 2's (player 1 as it now is), by hand; equal.
- `two_and_four_riders_are_deterministic`: two runs of 150 ticks hash the same at 2 and 4 riders; the
  last player is in the walk and moving it moves the hash.
- `stacked_riders_joust`: riders stacked 3 apart, 2 and 4 of them: every rider with one above comes
  off with reason 7 on tick 1, thrown at exactly the velocity the rider above had *before its own
  frame* (the order showing); the top one does not joust. Side by side, the head ray strikes the
  other rider but the hit clause answers nothing (level heights).
- `a_ninth_rider_is_refused`.

None of this is against the original: there is no two-player capture (R9 / plan/HUMAN.md).

## Not verified / not 1:1

- **The other rider's body shape is the port's own.** The original's rays strike the other bike's
  collidable, whose shape is its model's; the port loads no bike model, so a rider's body is a sphere
  about the body rays' hub (`air::HEAD.0`) as big as the head ray (≈2.63). Which contacts happen, and
  when, will differ from the original near the edges of that shape.
- **Only the head ray looks at other riders**, as only it looks at scenery (`ride::hits`, S37): the
  ten other body rays, and so the physical push between two bikes (the collision record's resolve,
  `contact.rs`' mutual case), are not ported. Riders pass through one another; only the wreck test
  sees them.
- **The mirrored entry is not written.** It cannot wreck the struck rider (their test skips kind 1),
  but it raises their record's touched word, which their airborne control and trick machine read as
  `touching`. In the original that word survives only until the struck rider's next collide clears
  it, and the port clears `touching` at the top of each frame anyway; whether a mirror ever reaches
  a read in the original was not traced.
- **One-per-ray nearest** for the bike's ray probe is assumed from the plane's (`Record::one_per_ray`),
  not read off the bike's probe. A tie between scenery and a rider goes to the scenery, and between
  riders to the first in player order — the port's choice.
- **A downed rider is not a rival.** While a rider is down the bike is the wreck body (`Crash`) and the
  rider a ragdoll; neither is offered to the others' head rays. The original would test whatever
  collidables the wreck leaves up; not traced.
- **The rival sweep is once a frame, at the frame's end**, like the scenery one (S37's composition);
  the original collides every substep.
- **The vehicle node's own parent** in the scene tree was not traced: the order rests on the players
  being made, appended and walked in index order, and the vehicle being made inside the player's
  initialiser.
- **Controls past two pads** are neutral from `poll_all`/`step`: the device manager has two ports
  (`input::PADS`), no multitap. Players 3–8 need `step_all` with their controls (online play, AI).
  A replay fixture still carries one `Input`, so a fixture's other players read their own (absent)
  pads.
- **A plane ride is one player's**: with a plane up only it is stepped, as before.
- **Spawn points** for players 2–8 are the caller's: where the original puts them is R9's question.
- Nothing here was compared against a two-player session of the original: no capture exists.

## Next

The task's code and tests are complete. When R9 lands: spawn points, the body shape (if the bike model's
collision mesh can be read), and a two-player capture to replay against.
