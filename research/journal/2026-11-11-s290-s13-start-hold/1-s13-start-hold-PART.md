# 1 — s13 start hold (PART)

Task S290. 2026-11-11. **Blocked after S304**: the hold is ported and proven, but it is not applied
to the riders in `Sim::race` yet. Applying it is only right together with the timeline's cadence,
which is S304's (see Next).

## What I did

- `crates/game-sim/src/race.rs`:
  - `race::hold(ch)` is the start hold on a `Chassis`: free 0,1,0, ang_free 1,0,1, the writes the
    timeline's arming makes on each item's body.
  - `StartStep::apply(&self, ch)`: `armed` → hold, then `warning` → `release`. This is the timeline's
    body writes, done to a rider's chassis.
- `crates/game-sim/tests/race.rs`:
  - two proofs (below);
  - the comment in proof 20 (S13f's image test) now names the writer of the turn freedoms.
- `research/oracle.txt`: race_countdown_listener's "another writer, not found" now names it.
- Not wired into `Sim::race_frame`, on purpose (see Next).
- Nothing taken from opentennis: it has no races.

## What the original does

**What pins the bike** is S13f's finding: the supercross countdown listener (`./kit dis 2fb9b0 --len
0x420`). On the countdown's zero it holds each MXSupercrossInterface item's body (on sx01, the human's
bike chassis). In the warning window it frees all six axes.

**The arming's call timing does not depend on the cadence.** In score_manager_tick (`./kit dis 0x25fdc0
--len 0x150`), with the countdown running (+0x5c bit 0x80), an interval > 0 and a listener:

- if acc ≥ interval, then acc −= interval and the listener is called;
- otherwise, if acc == 0, the listener is called.

Both cases come before `acc += dt`, and before the end check (`elapsed < length` → elapsed += dt,
else the countdown ends, 0x25fef8). The reset leaves acc and elapsed both at 0, so the first countdown
tick calls the listener with elapsed 0, and that call arms the hold. The warning call is different: it
falls on whichever accumulator boundary lands inside the 0.1 s warning window, so it needs the cadence.

**The other writer of the turn freedoms** (S13f's "not found") is bike_ground_step (0x257a00). Every
tick it calls `chassis_set_flags(chassis, 1, 1, !bike+0x574)` (oracle.txt S56 note; in the port,
`move::pose` → `chassis::set_flags` in the ride step). It only runs while bike+0x748 is 0. On S196's
image bike+0x748 is 0 and +0x574 (a wheel down) is set, so the arming's 1,0,1 becomes 1,1,0 on the next
frame. That is what the image holds. The linear freedoms have no other per-tick writer, which is why
the image still reads the hold's 0,1,0.

## How it was proved

`./kit check -- -p game-sim --test race -- start_hold held_bike start_timeline sx01_image`: all pass.
S196's image was symlinked into `context/states` from s2, so the image tests ran and did not skip. A
mutant `hold` (free 0,0,0) failed both new proofs.

- **the_start_hold_on_a_chassis_is_the_timelines_on_its_body**: 20,000 generated cases.
  - Windows at any 0.1 s step; clocks at zero, −0, a denormal, every window edge, a ulp either side,
    or anywhere.
  - Any starting freedoms, including values other than 0 and 1.
  - Each case runs `start_timeline` on an item and `StartStep::apply` on a chassis with the same
    freedoms. The two must agree on all six.
  - Steps that arm, that warn, and that do neither each occur more than 1,000 times. `start_timeline` is
    S13f's oracled port (proof 18), so `apply` matches the original's writes through it.
- **the_sx01_image_s_held_bike_turns_as_its_ground_step_sets**: S196's sx01 image.
  - The bike is on its ground step with a wheel down.
  - The image chassis' six freedoms equal `hold` followed by the ground step's
    `set_flags(1, 1, !on_ground)`.

## Not verified / not 1:1

- **The hold is not applied in a race yet.** `Sim::race_frame` neither holds nor frees riders. Holding
  at the countdown's zero is exact without the cadence, but the warning that frees the bike ~3 s before
  the drop is not. Holding without freeing would pin riders for the whole countdown, and the gate watch
  could never catch the creep that the original penalises in the warning window. That is worse than
  today. The gate watch (and `release` at the drop) are still the only things holding a rider. This
  waits on S304, which adds the cadence and runs the timeline.
- **No live capture of a start** going through the hold and the warning. The proofs are the oracled
  timeline (S13f) and one RAM image.
- **That the ground step is the only writer of the turn freedoms** between the zero and 3.59 s was
  checked on the image (its state and the values match) and in the code, not with a write watch over
  the countdown. No watch was run, because the image alone shows the ground step's preconditions hold
  and its output is what the image reads.
- **Which riders are items when there are several, and when there are AI riders**, is not proven. The
  image has one rider. `apply` is meant for every rider's chassis, as S304 will call it.
- **A computer rider's start message (bike_race_start) draws three or four times** (S12). This is not
  ported: the port's race riders are all players.
- **The drop's item word writes** (+0xc = 1, +0x10 = 0) are still not ported (S13f's note). What reads
  the words was not traced.

## Next

S304 runs the timeline on `Sim::race` with the score manager's cadence. In `Sim::race_frame`, apply
the returned `StartStep` to every rider's chassis with `step.apply(&mut rider.ride.chassis)`, after the
gate watch's pushes and before the drop's `release` (that is the original's order: the listener is
called before the countdown's end check). Then S290 is: the existing race tests, plus one that a rider
held from tick 1 cannot creep until the warning, and is caught in the warning window. Two existing
tests will need a look:

- `a_rider_rolling_through_the_closed_gate_is_thrown_back_and_loses_the_gas` uses a 30 s countdown
  with no windows;
- `the_countdown_drops_the_gate_and_lets_the_riders_go` sets the hold by hand.
