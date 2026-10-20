# 1 — s34 the biplane flies (FINAL)

Task S34. 2026-10-20.

## What I did

- Ported the plane's own four per-tick pieces into `crates/game-sim/src/biplane.rs` under
  `#![deny(clippy::float_arithmetic)]`: `controls`, `autopilot`, `Engine::step` and `Aero::forces`.
  Each is proved bit-exact against the original over 10,000 generated cases in
  `crates/game-sim/tests/biplane.rs`.
- Taught the oracle's interpreter two instructions the plane needs: cop2 `vabs` and MMI `pcpyh`
  (`crates/ps2-oracle/src/ee.rs`).
- Named what the proofs call in `research/oracle.txt` (block "S34"): `boot_gp`, `biplane_controls`,
  `biplane_autopilot`, `biplane_engine_step`, `biplane_aero_forces`, `body_force_local`,
  `body_moment_local`, `dynamics_world_step`.
- **Split the task** (`./kit plan add`; noted on S34). The rest of the brief, meaning the rigid body, the world
  step, the wheels, the crash test, the `impl Game` lane and the acceptance ratchet, sits on a body library
  the bike never used. It is too big for one session, and the acceptance run cannot be checked until C7's
  capture exists. The new tasks:
  - **S38**: the plane's rigid body (force and moment accumulators, the integrator, gravity), the
    world step with its substeps and constraint list, and the measure listener that feeds this module
    its `Attitude` and `Air`.
  - **S39**: the three tire listeners with ground and water contact, the plane's collision force, and
    the crash test that sets the wreck reason.
  - **S40**: wiring it all into one per-tick update, the plane as an `impl Game` lane, and
    `the_biplane_acceptance_run` with its ratchet at 0 against C7's capture.

## What the original does

The plane is a single body in the engine's generic dynamics world (`./kit fn 1f29a0`). Each frame the
world calls its *preforce* listeners once: the plane's controls are one of them (`./kit dis 352160 352460`),
and so is each tire's (`./kit fn 34ef80`). Then it runs `n = (int)(dt * 115.0) + 1` substeps (115.0 is in
the world object, at +4). In each substep it calls the force listeners: the plane's
(`./kit dis 352850 352960`) applies the collision force and then, unless the plane is wrecked (+0x3b0),
the autopilot, the engine and the aero. After that come the two tire force listeners per wheel. The
world integrates every body (vtable +0x44, then +0x30) and iterates its constraint list. Once the
substeps are done it calls the post listeners, and last the plane's measure listener
(`./kit fn 352960`). The measure listener rotates the body's velocity and rates into the body frame,
gets alpha and beta from the acos table, works out the Euler angles and the grounded flag, and these are
what the next frame's listeners read.

- **controls**: forward raises the throttle and back lowers it, at the file's increment per second;
  back is also the brake. With both held on the ground, the throttle jumps to the reverse throttle and the
  brake is released. The throttle is clamped to reverse..1. The elevator, aileron and rudder chase the
  pad times their maxima, each at its own rate. The elevator fades to nothing between the ramp-off and off
  roll angles, unless the nose is up steeper than 0.8. The tail wheel's steer chases the rudder buttons
  at the rudder's rate.
- **autopilot**: if the aileron stick is outside its dead zone, the elevator command is the pad's
  elevator times MaxElevator. Inside the dead zone, the commanded pitch rate is the stick's plus a banked
  turn's `g·tan`-style feed-forward (taken from the curve table, and dropped once the nose is above
  0.7070). A PI loop on the body pitch rate chases it, with the integral clamped.
- **engine**: power comes from the throttle with an altitude fade. Propeller efficiency grows linearly
  with speed from its minimum up to full. Thrust is power·550·efficiency/speed, applied along the body's
  nose through `body_force_local`. The prop's rpm and angle and the engine-note pitch (lifted by smooth
  noise, and more of it as the nose drops) are kept for audio and animation.
- **aero**: dynamic pressure comes from the density and the speed. The side force and yaw use
  `max(speed, 131.9994)` when the true-speed flag is clear. The six coefficients are summed from alpha
  (lift has a rounded stall: round-off, drop-off power, scale), beta, the three body rates scaled by
  `span/2V` or `chord/2V`, and the three surfaces. The result goes out as one body-frame force and one
  body-frame moment.

## How it was proved

Function oracle on `context/states/biplane.p2m2_SaveState` (the live plane at 0x00c44a40), 10,000
generated cases per function. Every input the function reads is perturbed around its shipped value or
drawn over its range, and every output the original writes or hands to the body is compared bit for bit:

- `the_controls_match_the_original`: every surface, the pad fields it rewrites, and the integral.
- `the_autopilot_matches_the_original`: the elevator command and the pitch integral.
- `the_engine_matches_the_original`: the force handed to the body's force entry (hooked), the
  power, rpm, prop angle, thrust, note and clock.
- `the_aerodynamics_match_the_original`: the force and the moment handed to the body (hooked) and
  the five stored coefficients.

Mutation check: changing STEEP_NOSE, inverting the true-speed flag, or moving the note's 0.1 threshold
makes three of the four tests fail. Restored afterwards. `./kit check -- -p game-sim --test biplane`
gives TESTS PASSED.

## Not verified / not 1:1

- **The plane does not fly in the port.** No per-tick update is wired, there is no `impl Game` lane and
  there is no `the_biplane_acceptance_run`. These are now S38/S39/S40. A harness naming a plane field
  still gets NoSuchField.
- **Nothing is replayed against a capture.** C7's biplane capture does not exist yet. The four functions
  are proved only in isolation, never over a ride.
- **Parameters come from the save state's RAM, not from `biplane.phy`.** The S33 loader has not landed.
  The fields are named after the `.phy` keys R8 lists, but the key-to-field mapping has not been checked
  against a parsed file.
- **The smooth noise is not ported.** The engine takes it as a closure. The proof hooks the original's
  noise on both sides with the same stand-in function, so the note is proved only given the noise value.
- **`MIN_DYNAMIC_SPEED` (131.9994) is a constant in the port.** In the original it is a RAM global, and
  I did not find the code that sets it. If anything changes it at run time, the port diverges.
- **The 0.7070 nose threshold (`TURN_NOSE`) is copied from the instruction's bits.** No generated case
  lands within an ulp of it, so the test does not tell it apart from a neighbour.
- **The generated input ranges are bounded.** `CLDropOff` stays in 1..10 because it is a `powf` exponent.
  Speeds, angles and rates are drawn over plausible flight ranges, not the whole of f32. NaN and inf
  inputs are not exercised.
- **The body's force and moment entries are hooked, not run.** The port returns the body-frame vectors.
  What `body_force_local` and `body_moment_local` do with them (rotate by the quaternion, accumulate, run
  their listeners) is S38's.
- **Unread:** the MX-specific post listener (`./kit fn 356260`) and the MX frame update
  (`./kit fn 354b20`). It is unknown whether they touch flight state.

## Next

Start S38 from `./kit fn 1f29a0` (the world step) and the body's vtable at 0x451f10: port the integrator
pair (`./kit fn 1f2340`, `./kit fn 1f20d0`) and `./kit fn 1f77c0` first, then the measure listener
`./kit fn 352960`. The tracer approach used here helps: run the world step under the oracle from the save
state and list every call target. The plane's world node is at 0x0060b200, with the world at +0x78.
