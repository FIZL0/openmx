# 1 — 360 flips

Task MD20. 2026-10-23.

## What I did

- `air::both_at_once(step)`: true when `BOTH_AT_ONCE_UNDER_R1` is on and R1 alone is held
  (`Step::modifier`, which only a trick-pack ride ever sets). While it holds, the flip lip test
  (`rotate`) no longer refuses under a 360 and the 360's lip test (`three_sixty`) no longer refuses
  under a flip, so a diagonal flick arms and fires both on the same tick, and either can fire while
  the other waits on the lip, until its grace ends or the wheels leave.
- New `Step::spin_active` (= `Spin::active`, set in ride.rs next to `spinning`). The flip lip test now
  refuses under a triggered 360 (`active`) as well as one that is turning (`applied`). M21 says MX vs.
  ATV refuses on "applied or active"; MD16 only checked applied. Without R1 this is the exclusive rule.
- Nothing else changes in how the moves run: each keeps its own torque and decay, both moments go in
  every substep, each ends on its own test, each landing test stands (360 clip past `SPIN_CLIP` →
  Tail wreck; a flip that isn't upright → the backflip's ground pitch wreck). Scored as both: each
  move's own node and points. No combination row, no bonus.
- `SPIN_PITCH_FOLLOWS_THE_FLIP` + `Spin::forward`: a 360 that leaves the ground with a front flip turns
  the sign of its pitch moment (nose-down). The torque value and its decay are left as they are.
  Without this the 360 front flip can't be done: the 360's pitch share (−600..−1100 on the generated
  bike) almost cancels the front flip (1000..1400). Measured on the generated lip: the bike yawed round,
  its up axis never went under 0.75, and the front flip never went over.
- `Spin::forward` is in the snapshot wire list and the trick-pack hash walk.
- Tests (crates/game-sim/tests/ride.rs): `md20_three_sixty_flips_turn_both_ways_and_land_clean` (360
  backflip and 360 front flip, each turning right and left), `md20_either_starts_while_the_other_is_triggered`
  (360 first, backflip 6 ticks later, and the other way round; without R1 the backflip is locked out),
  `md20_one_short_of_either_wrecks` (backflip stopped short → Front wreck on landing, with the 360
  scored and the backflip not; 360 not ended → Tail wreck from clip 2 while the flip is still going,
  for the backflip and the front flip). `md16_the_pump_without_r1_is_the_original` is extended: the
  diagonal pump with R1 up is bit-equal to the original, and on Original MD20's R1 hands start no 360.
  To measure turns the tests use body-frame quaternion steps (`md20_turned`), as M21 did: `came_over`
  reads a 180° yaw as a pitch turn.

## What the original does

openmx-mods M21 (`notes/tricks.md` "Both at once"): in MX vs. ATV the 360 and the backflip are
exclusive on each jump. Each lip test refuses while the other is applied or active, a same-frame tie
goes to the backflip, only one torque goes in per frame, and neither trick table has a combination row.
MX Unleashed has no 360. So the combination isn't in either game. `notes/openmx-asks.md` asks for it to
sit behind a switch with named constants and for the accurate rule to stay exclusive.

Decision: the brief asks for the combination under R1 on MX vs. ATV tricks. I followed the brief and
made it a switch: `BOTH_AT_ONCE_UNDER_R1` (doc comment says it is the port's own; `false` gives M21's
exclusive rule on every ride). I did not put it in `air::INVENTED`, because that is a list of extra
rotation rows (the flat spin) behind the invented-tricks setting, and the combination is a rule
between two moves, not a row. Without R1 the exclusive rule holds, so the
accurate behaviour is still reachable on the same ruleset.

## How it was proved

Generated rides in `crates/game-sim/tests/ride.rs` on the disc 125 with MD16's generated pack
(`MD16_TABLE`/`MD16_PHY`) on the generated `FLIP_RAMP`. 4 clean combinations, 2 grace orders, 1
no-R1 lock-out, 3 short cases, and the extended no-R1 / Original bit-equality. I found the hands by
sweeping the end-of-move stick windows. Clean landings came from windows starting at tick 220 and
lasting 30 to 50 ticks for all four. Gate: `./kit check`.

## Not verified / not 1:1

- The combination itself: neither game runs both moves on one jump (M21), so nothing exists to
  compare it against. It is the port's own by design.
- `SPIN_PITCH_FOLLOWS_THE_FLIP` (nose-down cork with a front flip) is invented. The front flip is the
  port's own already (MD16), and MX vs. ATV's 360 only ever pitches nose-up.
- The tests use the generated pack on the disc 125 and skip without the disc. No MX vs. ATV pack
  file was ridden.
- The extra "refuses while the 360 is active" in the flip lip test is taken from M21's notes. It was
  not stepped against the MX vs. ATV binary here.
- A 360 alone already scores the backflip watch (24): its cork takes `up_y` below 0. This is MD16
  behaviour, not changed here, and it means a 360 backflip's node 24 is not proof by itself that the
  flip finished. The tests also check the pitch turn.
- The "one short" front flip case: stopping the front flip early (same windows as the backflip case)
  also ends the 360 badly, so the clean "flip short, 360 home" case is tested only for the backflip.
  The front flip short-of-the-360 case is tested.
- The snapshot format gained `Spin::forward` without a version bump, which follows how MD16 added
  `Spin`.

## Next

Nothing for this task.
