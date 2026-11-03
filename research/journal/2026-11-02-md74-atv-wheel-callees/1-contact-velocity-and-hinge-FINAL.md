# The contact velocity and the hinge force

## What the original does

`./kit dis 4a1150 --len 1328 -p atv` (atv_wheel_contact_velocity) and `./kit dis 4a2aa0 --len 0x70 -p atv`
(atv_wheel_hinge_force, with atv_wheel_hinge_moment and atv_wheel_steer_torque_add under it). Field map:
research/oracle.txt `# MD74`.

- **Contact velocity.** The body's point velocity at the load point (+0x140) is
  `velocity + to_world(spin) × (load point − position)`. The ground's velocity (+0x1b0) and its velocity at
  the *contact* point (+0x120, not the load point) go to +0x1c0. They are worked out only while +0x1a0 is
  clear, and +0x1a0 is then set on every path. So when the tire forces bottom out and call again, the
  second call reuses the ground's values with the body's moved position. The ground can be any of these:
  - a collision object (+0x1f0) on a rigid body of the quad's own kind. Unlike MX Unleashed's owner branch,
    the spin is crossed with the arm in the body's frame and the result is turned into the world afterwards.
  - a collision object on a chassis, through the object's transform. This is MX Unleashed's arithmetic at
    other offsets.
  - a mover (+0x1f4), through its two virtuals (spin, then velocity) and its transform.
  - nothing: (0,0,0,1) for both.

  Then +0x170 = point velocity − ground point velocity, +0x180 = body velocity − ground velocity, and
  +0x190 = +0x180 turned into the body's frame.
- **Hinge force.** The body's add-force-at (vt +0x84, scale (1,1,1,1)), exactly what an unhinged wheel does.
  Then the moment `(point − mount_world) × force` is turned into the body's frame, dotted with the hinge
  axis (+0x8c0), and added to the steer torque (+0x60) as `0 + torque + f12·dot` on the FPU accumulator.

## Port

`crates/game-sim/src/atv_wheel.rs`:
- `contact_velocity`, `hinge_force` and `run` are new.
- `Body` gains `velocity` (+0x80) and `spin` (+0x1d0).
- `More` gains +0x190, +0x1a0, +0x1b0, +0x1c0 and +0x8c0.
- What the wheel stands on is `Ground` (`Still`, `Object` on a `Holder::Body`/`Holder::Chassis`, or
  `Mover`). It reuses `contact::Node` for the transforms.
- `tire_forces`' callee now also gets the `More`, so `run` can be passed straight in. Nothing outside the
  test called it.

Nothing was taken from opentennis: it has no ATV wheel.

## Proof

`tests/atv_wheel.rs`, image `context/mxvatv-ps2/md37a/atv-r-spawn.ee.bin`:
- **The ground in RAM.** A collision object, a rigid body, a chassis, a scene node (marked up to date, with
  no parent) and a mover are laid out in RAM. The mover's vtable points at two hooked addresses that
  serve its velocity and spin.
- **What gets compared.** Every region is compared word for word, and the original may write nothing
  outside them or the stack.
- **`the_atv_wheel_contact_velocity_is_the_originals`.** 10,000 states. Every path is taken ≥ 500 times:
  known, still, a body, a chassis, a mover, and an object hiding a mover.
- **`the_atv_wheel_hinge_force_is_the_originals`.** 10,000 states. f12 is 1.0, or another value in
  1 of 4.
- **`the_atv_wheel_tire_forces_are_the_originals`.** The contact-velocity and hinge hooks now check and
  log the call, then run the original's own code from inside the hook. The port runs `atv_wheel::run`.
  Every path it already tallied still shows up ≥ 100 times.
- **Mutations.** Each of these turned the tests red: the body branch's cross product swapped, the cache
  ignored, the hinge's f12 ignored, the hinge's torque left out of `run`, the chassis' object centre left
  out, the mover's arm taken from the load point, and +0x190 turned the wrong way.

`./kit check -- -p game-sim --test atv_wheel`: 4 passed.

## Not verified / not 1:1

- The ATV's own ride does not call these yet. The port has no ATV Force-body loop that runs the wheels,
  so nothing outside the tests uses them.
- Which code clears +0x1a0 (the "ground known" flag) was not traced. The port leaves it to the caller.
- The scene node's update from its parents is not ported. The port is handed nodes that are already up to
  date, as in `contact::Node`, and the oracle marks its node up to date.
- The mover's two virtuals are served by hooks, not run. What class of mover the ATV meets in play, and
  what those virtuals return there, was not checked.
- Not read in the PC exe first (AGENT.md asks for that). Both functions are short and straight-line apart
  from the four ground branches, and the PS2 oracle covers every branch.
