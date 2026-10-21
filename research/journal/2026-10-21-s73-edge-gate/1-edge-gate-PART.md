# 1 — edge gate (PART)

Task S73. 2026-10-21.

## What I did

- Decoded the bike builder's map-edge gate and its fourth argument (below; also in `research/oracle.txt`
  under `bike_edge_armed_by`, S73).
- `game_sim::off_map::{Built, League, armed}`: who the bike was built for (player, AI, ghost) and the
  event's league (supercross, nationals, free ride) → whether the edge clause is armed.
- `Rider::edge_armed`, decided once when the bike is built, like the stunt set: `Rider::new` sets it
  for a player's bike on a free-ride area (the only event the port has), so every rider the shell
  builds today is armed, as before. `Rider::frame_among` now gates S59's clause on it instead of on
  `Sense::owner.human`. The original's clause reads only the bike's word, never the owner.
- Snapshot: the arm is a session constant. It is not sent and not hashed, so no wire format or pinned
  hash changes. `decode` puts this machine's own back, as it does for `body_numbers`.
  `tests/hash.rs`'s `Sim` size pin goes from 9744 to 9752 for the new field. The leaf count is unchanged.
- opentennis: nothing to take (a tennis game has no map edge).

## What the original does

`./kit fn 0x0027aa70`, the gate at `./kit dis 0x0027ac4c 0x0027acac` (bike in s3, a3 in s1):

- a3 == 1: the word is 0.
- otherwise the event's first word, the league (R7: 0 supercross, 1 nationals, 2 free):
  1 or 2 → `a3 == 0`; 0 → 0; anything else leaves the word alone. The vehicle base constructor
  (`./kit fn 0x0024f420`) clears it, so "alone" means 0.

a3 comes from the builder's only caller, the vehicle loader's bike spawner (`./kit fn 0x0020e0b0`, called
once from `./kit fn 0x00208c50`). It passes a3 through from a per-player array that the loader fills
from the player's kind (`player+0x34`, which `player_init` writes):

- kind 3 → 2: the ghost that `spawn_racers` makes for activity type 3, the ghost race.
- kind 2 → 1: the AI.
- kind 1 → 0: a human, pad i.

Players whose `+0x6c0` is 1 or 2 are skipped and get no bike from this loop.

So the clause is armed only on a human's bike at a nationals or free-ride event.

## How it was proved

- Function oracle, `crates/game-sim/tests/edge_armed.rs::the_builders_gate_is_the_originals`: the
  original's gate itself, run in the EE interpreter over 12,000 generated (a3, league, prior word)
  cases. The armed word matches `off_map::armed` on every in-range case: 1334 armed and 4940 disarmed.
  The 5726 out-of-range words pin the original's answer, 3211 of them "left as found". It ran and did
  not skip; the counts are in the log.
- `a_disarmed_bike_rides_past_the_edge`: a new rider past the margin is thrown, and the same bike stays
  unthrown for 30 frames when built for the AI, a ghost or a supercross event.
- S59's `off_the_map` and the snapshot tests are unchanged and pass.

## Not verified / not 1:1

- **The league is fixed at free ride in the port.** The port has no events and rides only the five
  free-ride areas, so `Rider::new` always arms. A supercross or nationals event would need the shell to
  set `edge_armed` from `off_map::armed`. Nothing does that, because no such event exists yet.
- **The port has no AI or ghost-race riders**, so `Built::Ai` and `Built::Ghost` are reached only by
  tests. Every rider the port builds (local seats and netplay peers) is a player's. The original has
  no online play, so treating a remote peer as a "player" (a3 = 0) is the port's own call.
- **No live capture for this task.** The kind→a3 mapping was read statically from the loader, not
  run. S59's RAM reads (the player's bike 1, the AI's 0, in rec-start and acc-start) agree with it.
  Not checked live: a ghost-race ghost, a supercross bike, and a nationals bike.
- **Other writers of `+0x570` were looked for with a text search of the decompilation** (none on the
  bike outside the constructor and the builder). A write through a computed pointer would not show up
  in that search.

## Next

none
