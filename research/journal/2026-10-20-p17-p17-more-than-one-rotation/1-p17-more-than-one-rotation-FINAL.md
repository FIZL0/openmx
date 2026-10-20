# 1 — p17 more than one rotation (FINAL)

Task P17. 2026-10-20.

## What I did

- `crates/game-sim/src/air.rs`: a rotating move is now a `Rotation` row — `Gesture` (which stick half,
  which way fires; arming is the other way past `HELD`), `Axis` lane and sign, `Tuning` (min/max time,
  min/max torque, the two decay rates) read through `fn(&Params) -> Tuning`, the fire `floor` (1.25),
  the `grace` (0.6 s), `over` (what gives control back besides the wheels touching) and `authority`
  (the ramp length). One step, `rotate`, runs any row; `Rotations` is a set plus which row is armed or
  turning.
- `BACKFLIP` is the disc's row and reads exactly the `Params` keys the old step read
  (`min/max_backflip_time`, `min/max_backflip_torque`, `backflip_torque_rate_increase/decrease`,
  `control_authority_time`, the two airborne pitch wreck angles). `air::backflip` keeps its signature and
  is now `rotate` over `[BACKFLIP]`, so the existing oracle proof proves the generalised step.
- `FLAT_SPIN` (yaw, positive, lean left then right, borrowing the backflip's tuning) is the one invented
  row, in `INVENTED`. Only `Rotations::new(true)` lets it in; `Rotations::default()` is `new(false)`, i.e.
  the backflip alone.
- Module head: a "Rotations: the disc's, and the port's" section listing which parts are which.
- Choosing among several rows: the original asks "arm?" before "fire?"; the general step asks "fire the
  armed row?" first. For one row the two can't both hold (one wants `< -HELD`, the other `> HELD` on the
  same value), so the answer is the same, and it is the order that lets two rows share a stick half in
  opposite directions. On a fire, any *other* lane a row of the set uses is zeroed so a spin's leftover
  can't ride along with a flip; with one row that clears nothing.
- Tests (`crates/game-sim/tests/air.rs`), the old ones untouched apart from the `use` line:
  `the_default_set_of_rotations_is_the_discs`, `the_flag_off_leaves_the_backflip_the_discs` (8000
  generated states, both flag values, against `air::backflip`), `a_flat_spin_is_a_row_the_flag_adds`
  (scripted arm/fire/take-off/turn with the flag on and off).

## What the original does

Unchanged from S10's port: one backflip step with its gesture (stick y), axis (pitch, negative) and
numbers written in. Nothing in the original reads a running trick to torque the bike (`trick.rs`'s head).

## How it was proved

- The backflip's own oracle proof, `the_backflip_step_is_the_originals` (8000 generated states, whole
  object graph compared against the original's step), unchanged and passing through the new `rotate`. It
  ran against the game (no skip line under `--nocapture`).
- `the_flag_off_leaves_the_backflip_the_discs`: `Rotations::new(false).step` and `Rotations::new(true).step`
  (lean held inside `HELD`, yaw lane starting clear) equal `air::backflip` field for field on 8000
  generated states each. Host-only.
- `./kit check`: TESTS PASSED.

## Not verified / not 1:1

- The acceptance run is not affected by construction rather than by a fresh comparison: nothing in the
  sim's substep calls `air::backflip` or `Rotations` yet (only the tests do), so there is no path for a
  difference to reach it. `./kit check` passing is the evidence; no separate replay was run.
- `FLAT_SPIN` is invented and proves only the *shape* — it has no original to match. Its tuning, grace and
  ending are the backflip's because the disc has no numbers for a spin; whether those feel right for a
  yaw rotation was not tried in play.
- Behaviour that the step keeps for every row but only the backflip's is the disc's: decay only while
  `dpad_dir == 0`, the lip conditions, and `airborne_control` silencing yaw and roll while `flipping`
  (that last one means a flat spin's rider keeps only pitch control, which is the backflip's rule applied
  to a yaw rotation — a port choice, not tested in play).
- With the flag on, firing the backflip zeroes the flip torque's yaw lane, which the original never
  touches. The equivalence test therefore starts the yaw lane at zero; that the game's own bike
  constructor leaves `flip_torque.y` at zero was not checked against a save state.
- With the flag on and both a lean and a pitch gesture past `HELD` at once, the earlier row in the set
  (the backflip) arms — an ordering choice of the port's.
- Wheelie and stoppie (`Trick::WHEELIE`, `Trick::STOPPIE`) were not made rows and not looked into: the
  air code's only rotation is the backflip, and where those two get their torque was not traced here.

## Next

Nothing for this task. A keyboard/pad binding for invented rows and a place to switch the flag (a mod
setting) are for whoever wires `Rotations` into the rider's substep.
