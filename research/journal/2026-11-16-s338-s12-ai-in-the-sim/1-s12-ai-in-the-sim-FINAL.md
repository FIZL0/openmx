# 1 — s12 ai in the sim (FINAL)

Task S338. 2026-11-16.

## What I did

C33's ai0 and ai1 now hold much longer in the s12e replay:

| Rider | Before (agrees through tick) | After |
|-------|------------------------------|-------|
| ai0 | 28 | 132 |
| ai1 | 28 | 147 (torn row 133) |

`C33_AGREES_THROUGH` in crates/game-sim/tests/ai/race4.rs is now `[132, 147, 1802, 206, 6]`.

- The fix is in the test only: `c33_player` maps each rider to the Sim player of its **game player number**
  (`c33_by_player`, read off the image: human 0, ai2 1, ai0 2, ai1 3).
- Before, it used the reverse of the actor list (ai0 3, ai1 2, human 1, ai2 0). So the Sim stepped ai0, ai1,
  human, ai2. The original steps ai1, ai0, ai2, human.
- No `src/` change. `Sim::step_all` already walks players from the last down, which is the original's
  order once the players are numbered as the game numbers them. `Sim::spawn_ai` (S331) already builds the
  lane in the game's player order, so the shell was not affected.
- Nothing from ../opentennis applied (no rider-on-rider contact there).

## What the original does

- At tick 29 ai0's body rays strike ai1's frame. The contact (kind 0 on ai0, its mirror kind 1 on ai1) is
  found by the collision node update from where tick 28 left the bikes. Point, normal, from and on-body
  match the port's bit for bit at ticks 29 and 30.
- The closing speed is about −7 ft/s, slower than the hard speed (−10). So it is a spring on each bike,
  in each bike's own update, over both substeps. No impulse.
- The riders update ai1, ai0, ai2, human. So ai0's spring reads ai1's chassis as ai1's frame has just left
  it (velocity x 0.158, not −1.277). Its normal speed is −1.7256, not −7.078. The damping term is
  36.6 × that, so the push is much smaller (Δforce x −442 vs the port's −638).
- ai1's spring reads ai0 before ai0's frame.

## How it was proved

- The bit-exact replay of C33 (`s12e_free09s_four_racers_ride_as_the_capture`): ai0 through 132, ai1
  through 147. Every agreed row is compared on the body's centre, quat, velocity and spin.
- The diagnosis came from temporary probes, removed before landing:
  - the port's contact lists before the step into ticks 24–31 (only 29 and 30 have one);
  - the AI bikes' collision objects in C33's image against `rider_contact::collider` (all fields equal);
  - S328's whole frame in Ee (draws.rs), with `collision_node_update` hooked to dump the four lists and
    `collision_resolve` hooked to log each call's rider, chassis velocity, rates, force, torque, normal
    speed and relative velocity, beside the same printed from the port's `Bump::resolve`.
- `./kit check -- -p game-sim --test ai s12e_free09s_four_racers`: passed.

## Not verified / not 1:1

- **ai0 parts at tick 133, ai1 at 148** (ai1's row 133 is torn). ai0 is near (4375, 152, 3436), coming
  down at −7.4 ft/s. Not traced. Filed as S347.
- **ai2 still parts at tick 207.** Not traced (S330's note stands).
- **The rule "update order = player number, highest first" is read off C33 only.** It is one race with
  four riders. It was not checked on another image or against the code that orders the update list.

## Next

- S347: trace ai0's tick 133 and ai1's 148 in the s12e replay, with the same interpreter probes.
