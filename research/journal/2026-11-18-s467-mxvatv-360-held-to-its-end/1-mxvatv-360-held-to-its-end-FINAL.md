# 1 — mxvatv 360 held to its end

Task S467. 2026-11-18.

## What I did

- `air::Spin` gets `starts` (the port's own): how many times this 360's clip has started. Zeroed when
  the 360 fires, bumped on each clip start (0 -> 1). On the netplay wire; not hashed (only animations
  read it, as `spin.cut` is left out).
- `Spin::shown()`: the 360's animation state runs while `clip != 0`, or once a start is counted and the
  360 is still active. So coming round no longer drops the rider and bike out to the air row; landing,
  a touch or the yaw floor (active false) does.
- `rider.rs` `spin_state` and `game-render` `vehicle.rs` `want_with_pack` use `shown()`.
- `vehicle_anim::Request::spin_starts` and `Driver::spin_starts`: a count that moves while a 360 state
  runs restarts it (fade from the saved pose at the 360's rate, clip from 0). The rider's body request
  and the bike's `Chassis::request` fill it. Driver's copy is on the wire; the request's is skipped
  (rewritten every frame before it is read).
- `mxvatv_stunt_cross.rs`: the tick-46 comment now says the zeroing clock is the remote bike's only,
  and tick 46 is checked against the original's phase too (1, as R47 found).
- Filed S478 (low): the R1-alone hold at 0.72 s and the bike's phase on its restart tick.
- Nothing from ../opentennis: it has no stunts.
- No live run of the original was needed; nothing in PCSX2.

## What the original does

Recipes: bike animate `./kit dis 0x004dd8f0 0x004dda60 -p atv`; the record copy `./kit dis 0x004ba940 0x004ba9b0 -p atv`.

- The restart ask (bike `+0x1650`) is copied each frame to the rider record `+0x1d0`, next to the stunt
  byte. The rider layer restarts on it: fade to 1, at the fifth rate (2/s). Probed: it does.
- The bike animate restarts on a new stunt id **or** the restart ask: fade 1 that tick, then 3/s.
- So the second start is the same cross as the first, for both.
- The answer to the brief's R1 question: yes, the original holds a turning 360's clip at 0.72 s while R1
  alone is held. The port does not yet. Filed as S478, because the hold's wobble (vt `+0x4ec`) is not read.

## How it was proved

Function oracle on state04's RAM image, the original's own code, tick by tick
(`crates/game-sim/tests/mxvatv_stunt_cross.rs`):

- `the_ports_rider_restarts_a_360_as_the_original`: the local trick step, the rider layer and the bike
  animate run 60 ticks in the 360 (past 1.5 s), then the stunt start's restart (clock 0, ask set for one
  tick) and 30 more. The port's rider driver, with the start count 1 then 2, matches the rider's fade
  and the trick step's phase bit for bit on all 90 ticks.
- `the_ports_bike_restarts_a_360_as_the_original`: the same for the bike's fade (80 ticks); the phase
  against the clock over the clip length, as MD248's bike test does.
- `a_360_shows_from_its_clip_start_to_its_end`: `Spin::shown` and `starts` through start, round, second
  start and end.

`./kit check -- -p game-sim --test mxvatv_stunt_cross`: 9 passed (image present; the earlier probe printed
its values, so they did not skip).

## Not verified / not 1:1

- **Not watched live.** No 360 was ridden in PCSX2 or in the shell to see the rider stay in the clip
  through the round. The tests drive the driver with the counts poked in.
- **The whole frame was not run together.** The port's `starts` comes from `spin_clip`, and the original's
  restart from its stunt start on the second pass (R47's reading of F31b5g). The two were not run side by
  side on one ride.
- **The bike's phase on a restart tick.** The original's bike animate plays phase 0 on the tick it
  restarts; the port plays dt / clip length (as on the first start, from MD248). Filed in S478.
- **R1 alone held through 0.72 s.** The original freezes the 360's clock and clip there; the port does
  not. Filed in S478.
- **Wire growth.** `Driver::spin_starts` adds one byte per rider to the snapshot. Its effect on the
  netplay tests is whatever the gate shows.
- **Only state04 and the right 360.**

## Next

S478.
