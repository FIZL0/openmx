# 1 — the biplanes body (FINAL)

Task S38. 2026-10-20.

## What I did

- Ported the rigid body the plane flies on, the dynamics world's step and the plane's measure
  listener into `crates/game-sim/src/biplane.rs`, still under `#![deny(clippy::float_arithmetic)]`:
  - `Body`, with every field the step touches: `add_force`, `add_force_local`, `add_moment_local`,
    `push_gravity`, `clear_shock_peak`, `integrate` (with the linear half), `end_substep` and `pose`.
  - `World { iterations, rate }` with `substeps` and `step`. The step runs over a `Dynamics` trait,
    one method per list element call, so the caller owns the lists (S40 wires the plane's).
  - `Measure`, in two halves: `contacts` (grounded, was grounded, touchdown, the aero model's
    true-speed flag) and `flight` (airspeed, body-frame velocity, world rates, alpha, beta, Euler
    angles, the matrix with its forward and up rows). `attitude` and `air` build S34's inputs from it.
- The body reuses `chassis::turn`. It also reuses `chassis::normalize`, which I made `pub(crate)` (that is
  the only change in `chassis.rs`): it is the same instruction sequence as the end of the plane's
  integrator.
- Added seven oracle proofs to `crates/game-sim/tests/biplane.rs`, 10,000 cases each, on
  `biplane.p2m2_SaveState`. All of them are bit-exact.
- Named everything the proofs call in `research/oracle.txt` (block "S38"): `body_integrate_linear`,
  `body_integrate`, `body_end_substep`, `body_add_force`, `body_wrapper_force`,
  `body_wrapper_preforce`, `body_wrapper_post`, `dynamics_world`, `mxbiplane_post`,
  `touchdown_sound`, `sound_trigger`, `transform_set_pose`.

## What the original does

**The world step** (`./kit fn 1f29a0`; world in a0, dt in f12). The world is embedded at world node
0x60b200 + 0x78. If its body count (+0x50) is zero, nothing runs at all, not even the listeners. The
test is `beqz`, so a negative count passes it. The order is:

1. The preforce list (+0x98): each element's `(*elem)+8` is called with `(elem, dt)`.
2. `n = cvt.w.s(dt * rate(+4)) + 1` substeps of `h = dt / (float)n`. At 30 Hz with 115 that is 4.
3. Every substep runs four things in order:
   - the force list (+0x68): `(*(elem+8))+0xc` with `(elem, h)`. The plane's elements go through an
     adaptor (`./kit fn 1f7770`) that forwards to `*(elem+0xc)`'s +8.
   - each body: if `*body != 0`, body vt +0x44 (integrate) with h, then vt +0x30 (end substep).
   - the constraint list (+0x80): if its count is positive, `world+0` passes. Each pass calls
     `(*(elem+0x10))+0x18` with `(elem, pass == 0)`.
4. The post list (+0xb0), with `(elem, dt)`.

Every count is read again after each call. In the state the world has iterations 1, rate 115.0, 3
bodies (the plane's and two others, all with the same vtable 0x451f10), 3 force, 3 preforce and 3 post
listeners, and **0 constraints**: in this state nothing constrains the plane.

**The body** (vtable 0x451f10). The integrate slot `./kit fn 1f2340` first calls the linear half
`./kit fn 1f20d0`:

- It copies the position to +0x50 unconditionally.
- Unless the inverse mass (+0x10) is `== 0.0`, it writes `(h, (0.5*h)*h, inv_mass)` to +0xf0..+0xf8,
  then computes `a = F * inv_mass`, `x += a*h²/2 + v*h` and `v = a*h + damping*v`, and stores a in
  +0xa0.

The rotation half then copies the quaternion to +0x150. It returns if the inverse inertia equals
`(0,0,0,1)` bitwise (`pceqw`/`ppach`). Otherwise it takes the same second-order step on the
quaternion: `q' = (q ⊗ ω)·(½,½,½,-½)` and `q'' = (q ⊗ α)·… − q·|q'|²`. It adds `q''·h²/2` and then
`q'·h`, normalizes through `vrsqrt`, and sets `rates = α·h + ang_damping·rates`, where
`α = inv_inertia·moment`.

The end-of-substep slot `./kit fn 1f77c0` keeps the per-axis peak magnitude of +0xc0 in +0xb0, and of
+0x1e0 in +0x1d0. It uses `c.ole.s` and branches on false, so an unordered pair also replaces the
peak. It then copies the force and the moment to +0x80 and +0x1a0, and sets all four accumulators to
`(0,0,0,1)`.

The force/moment entries are:

- force local `./kit fn 1f7a10`: `turn(q, f)`, then vt +0x38, which is `./kit fn 15afc0`, the
  world-frame add;
- moment local `./kit fn 1f3360`: added as given.

Both then call the body's own listeners (+0xd0), of which the plane's body has none.

**Gravity** is the plane wrapper's (plane+0x50) force listener `./kit fn 15af40`. If the body is
enabled, it calls body vt +0x64 (which forwards to +0x38) with wrapper+0xf0 = (0, −1499.9999, 0, 1).
Then, if wrapper+0x120 (the collision record) is set, it calls that record's vt +0x80 with 1, which is the
collision response `./kit fn 270dc0` and belongs to S39. Last it sets wrapper+0x1b0 = 0. The wrapper's preforce `./kit fn 15b050` clears the body's +0xb0.

**The post listener** is `./kit fn 355290`: the measure `./kit fn 352960`, then three MX copies
(`354e00`, `355000`, `355170`) that feed the recording and the rider. The measure runs these steps:

1. **The wrapper's post** `./kit fn 15b070` (the pose). It takes
   `origin = last_position − turn(last_quat, cofg)` and hands `(last_quat, origin)` to
   `./kit fn 1b21d0` when the actor's vt +0x10c says so.
2. **The grounded flag.** `+0x594 = +0x590`, then `+0x590 = any tire's +0x88 != 0`.
3. **The touchdown sound.** On a rising edge, and only if +0x67c is set, `./kit fn 1b30e0` gets the
   body's shock peak (+0xb0) and `./kit fn 3dd0a0` plays it.
4. **The true-speed flag.** The aero model's +0x30 is set to `grounded || collision+0x40`.
5. **Animation and crash test.** The plane's vt +0x1b8 (animation, S35) and +0x1bc (crash test, S39)
   run.
6. **The measurements.** These are:
   - the airspeed: `sqrt` of the FPU `mula/madda/madd`;
   - the velocity turned into the body frame (+0x5c0), a conjugate rotation done by swapping
     operands;
   - the world rates (+0x5b0);
   - alpha (+0x59c) from `(z, y)` and beta (+0x5a0) from `(z, x)`. Both use
     `acos(along · rsqrt(across² + along²))` off the arc cosine table, with the cosine clamped twice
     and the result negated when `across > 0`;
   - the Euler angles (+0x5f0, `quat_to_euler`);
   - the matrix (+0x600, `vu_quat_to_mat3` with the origin), using the *current* position less the
     turned cofg;
   - its rows 2 and 1, copied to +0x5d0 and +0x5e0 (`xyz` only).

## How it was proved

Function oracle, 10,000 generated cases per function, on `context/states/biplane.p2m2_SaveState`. Each
proof runs on the live plane's own body, except the world step's, which runs on a world built in
scratch memory. `cargo test -p game-sim --test biplane` (through `./kit check`) passes 11 tests: S34's
4 plus these 7.

- `the_body_integrates_like_the_original`: the whole body is generated, with pinned bodies,
  `(0,0,0,1)`/`(-0,0,0,1)` inverse inertias, ±0 inverse mass and junk `w` lanes. Every field is
  compared.
- `the_body_ends_a_substep_like_the_original`: lanes are drawn from {0, −0, x, −x, anything}, so
  ties and signed zeros come up.
- `local_forces_and_moments_match_the_original`: through the real vt +0x58 / +0x5c, and +0x38.
- `gravity_is_pushed_like_the_original`: the collision response is hooked and counted.
- `the_pose_is_put_like_the_original`: the actor's virtuals are hooked, and the origin is read where it
  is handed over.
- `the_measure_matches_the_original`: the wheels, the was-grounded flag, the touching flag and the
  sound pointer are all varied. The animation, the crash test and the pose are hooked out. Every
  measured field, the 4×4 matrix and S34's `Attitude`/`Air` are built from the result.
- `the_world_steps_like_the_original`: the five lists hold hooked elements that log
  `(which, argument bits)`, and listeners grow their own lists mid-frame. Iterations range over
  {−1, 0, 1, 2, 3}, rates over 1..300, counts over {−1, 0..3}, and some bodies are disabled. The
  original's call log equals the port's, call for call.

Mutation check: swapping the order of the two quaternion terms fails the integrator proof, and
flipping the first-pass flag fails the world proof.

## Not verified / not 1:1

- **Stale `w` lanes.** The original stores both accelerations (+0xa0, +0x1c0) as whole quadwords whose
  `w` is whatever a VU register held. The linear one's `w` is never compared. The port writes the
  angular one's `w` as the position's `w`, which is proved only when the linear half ran. With a
  pinned body the original's value is a caller's register, and the port keeps the old one, unproven.
- **The body's own listeners** (+0xd0, called by every add) are not modelled: the plane's body has
  none in the state. A wrecked or attached plane could have some; not checked.
- **Constraints.** There are none in the state, and the port runs the passes but has no constraint
  type. Whether any mode ever adds one is **unproven**. The world's constructor reserves the list
  like the other four, and the step and the destructor are the only readers I found. The adds are
  inlined (the list's grow helper is called only from the constructor), so a static search found no
  add site. Watching world+0x80's count with PINE through a crash and a mode switch would settle it.
- **The other two bodies** in the world (0x196f020, 0x18c05c0; same vtable) are not identified. The
  bike's, probably, but unchecked.
- **The actor transform virtuals** that follow the pose (vt +0x128/+0xdc when +0x10c says no, +0xd8,
  transform_touch, +0x110) and `transform_set_pose` itself are not ported. Only the pose they are
  handed is proved.
- **Not ported here, by ownership:**
  - the collision response (`270dc0`, called from the gravity listener), the tires and the crash test
    (+0x1bc) belong to S39;
  - the surfaces' animation (+0x1b8) belongs to S35;
  - wiring the listeners into one tick belongs to S40.
- **The touchdown sound** (`1b30e0`/`3dd0a0`) and the MX post copies (`354e00`, `355000`, `355170`)
  are not ported. The port reports the touchdown edge and nothing more.
- **Measure ordering.** The port's `Measure::flight` must be called after the crash test, which may
  change the body. Splitting the measure in two is the port's own structure, not the original's.
- **Inputs not exercised:** NaN, infinities and denormals (the PS2 FPU has none, but a junk save
  could hold such bits); `World::substeps` at a dt·rate past `i32`'s range.
- **I edited the plan text of S38 and S39** (`plan/TODO.md:81-82`). Once this block named their
  addresses, `./kit audit` counted the briefs' bare addresses as leaks, so the briefs now use the
  names instead. The meaning is unchanged.
- **Not compared against a live capture.** This is oracle-only; the acceptance run is S40's.

## Next

S39 (the tires, the collision response, the crash test), then S40 (wiring and acceptance).
