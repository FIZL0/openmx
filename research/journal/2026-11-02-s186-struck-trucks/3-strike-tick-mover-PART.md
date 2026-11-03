# 3 — what the bike meets on the strike tick (PART, blocked)

Task S251. 2026-11-03.

The task asked for the woken truck's `f.col` to be filed as a body in the bike's frame, with the path
mesh stood down, so that S207 ctruck and S219 rear agree past the strike tick. I built that, and it
made both rides worse. Then I recorded the original's contact list on the strike tick. **The original
meets the truck through the path mesh, as a mover, not through the `f.col` as a body.** The port
already builds those two contacts bit for bit. What parts the rows is the mover's velocity: the
original reads the car's motion *after the wake moved its node*, and the port's wake does not put the
node where the original's does. I have not found why, so the task is blocked.

## What I did

- **The f.col path, built and set aside.** `truck_contact.rs` files each physical truck's `f.col` at
  `PhysicsCar::pose()` for the body rays (category `0x18`, the props' Narrow), with mirrors on the
  truck's record and a `Bump::trucks` list resolved after the props. `StoodDown` hides a physical
  car's path mesh for the bike's part of the tick. Strike-tick results on the two rides, vel.x for
  ctruck (original 2.97) and for rear (original 19.30; it wrecks 4 on the tick):

  | variant | ctruck vel.x | rear vel.x | rear wreck |
  |---|---|---|---|
  | HEAD | 9.53 | 19.17 | (42, 4) |
  | f.col replaces the mesh entry, mesh stood down | 18.05, wreck (36, 1) | 12.07 | None |
  | f.col appended | 18.46 | 12.97 | — |
  | no mover on the mesh entries | 19.20 | 13.50 | — |
  | no f.col on the wake tick | 1.34, wreck (37, 1) | -0.52 | — |

  None is closer than HEAD. The code is on the local branch `wip/S251-fcol` (commit `ae1d5e14`, not
  for merge; it still carries the `S251_V`/`S251_B`/`S251_DEBUG` switches).
- **The original's list on the strike tick.** `watch/s251-truck-list.toml` records the bike's record
  (B7's lanes on the S89 rider chain) and the truck car's node, `+0x2e0` copy and `+0x260`/`+0x290`.
  It was recorded from copy 2's slot 25 (S219 rear) by `research/s219/record.sh`, giving
  `context/capture/s251/rear-list.csv` (frames 18482..18541, the same frames and lanes as S219's
  `rear.csv`). The count reads 0 after the frame, as in B7, but the entries stay in memory. On tick
  42 the last list built has two entries. Both are kind 0, other 0, mover the truck car, with normal
  (0.70539, 0.21341, 0.67592). The points are the port's two `car_contacts` entries exactly.
- **The truck's cached motion.** `+0x260` and `+0x290` are 0 up to tick 41. From tick 42 on they read
  (26.530971, 14.010800, 31.218940) and (-3.515880, 1.0954884, -3.653163), so the velocity virtual
  was first asked on the strike tick. That velocity is (tick 42's node − tick 40's node) × 30.003
  on all three lanes, to 5 digits. The port's mover on that tick is the path's (20.714962, 4.9191623,
  18.817799), with a near-zero spin.
- **Feeding the wake's motion to the mover (not kept).** I had `Traffic::strike` set the woken mesh's
  `moving` to the wake's `Moving`. The port's wake then gives (16.715538, 4.5927444, 15.133348), with
  the node at (3774.7458, 246.65788, 3232.182) against the original's (3775.0729, 246.97178,
  3232.7180). Rear got worse: (14.10, 8.57), and no wreck. I also ran the car's path update for the
  tick before the wake: the node went to (3775.4346, 246.82152, 3232.8074), the velocity was 16.66,
  and rear was no better.
- **The truck does not stop dead.** Its node goes on 0.5–0.8 ft a tick after the strike, and climbs.
  S219's "stops dead" read the path car's own position, which is frozen while the car is physical.

## How it was checked

PCSX2 copy 2 (mine, started and stopped by pid), and the port's replay
`cargo test --release -p game-sim --test replay -- a_bike_meets_a_moving_truck` with a debug print of
the bump's contacts and movers (on the wip branch). Before the strike, the capture's lanes equal
`context/capture/s219/rear.csv` (vel, wreck 4 on tick 42). This branch adds only the spec, this
journal and the oracle block. No code changes, so no test changed.

## What would unblock it

What the original's `physics_car_wake` (0x003e3b00) does to the node when it is called from the bike's
search on tick 42. Running it in the interpreter on a RAM image of frame 18523 (S219 rear, one tick
before the strike) would show the placed node and vt+0xb8's answer. Then compare those with the
port's `PhysicsCar::wake` inputs: `car.pose`, `head`, `normal` and `last` at the moment
`Sim::car_contacts` calls `Traffic::strike`. A PCSX2 savestate one frame earlier, or a
`record.sh`-style capture with `+0x2e0`, `head` and `normal` logged per substep, would do as well.

## Not verified / not 1:1

- The stale entries are taken to be the strike tick's last list. The count is 0 when sampled, so how
  many entries the list held is inferred from the kind/mover pattern (entries 2..5 are the earlier
  garbage that never changes).
- Whether a later tick does meet the `f.col` as a body (the task's premise) is not seen: rear wrecks
  on the strike tick, and the ctruck state (copy 3, slot 35) was not recorded with this spec.
- Which call left `+0x260` at its value (the wake's or the bike resolve's) is not known; both read
  the same node unless something moves it between them.
- Why the port's wake places the node about 0.5 ft from the original's is not found. S186's
  interpreter proof covers the wake's arithmetic on s89-along, not its inputs on this tick.
