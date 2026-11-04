# The rigid body and the world step (PART)

## What the original does

Read with `./kit fn` / `./kit dis -p atv`. The field map and every row are in research/oracle.txt `# MD75`.

- **The world step** (atv_world_step) has MX Unleashed's layout and order (`biplane::World::step`). The
  one difference is that the constraint call also gets the substep's `h`. The game's world (atv_world)
  has 1 iteration at 115 a second, so a frame at 1/30 runs 4 substeps. Its lists:
  - bodies: the quad's body alone;
  - force: one wrapper (atv_world_force_wrapper) that hands each substep to the quad's Force interface;
  - constraints: none;
  - preforce: six listeners. These are the quad's PreProcess, the second ATV's, and four more objects of
    one other class;
  - post: the quad's PostProcess.

  The quad's three interfaces at +0x30/+0x34/+0x3c are one-jump thunks to atv_quad_ground_head,
  atv_quad_air_state and atv_quad_ground_step.
- **Integrate** (atv_body_integrate) is a later build of MX Unleashed's body. It is *not* the biplane's
  `Body::integrate`.
  - The linear half (atv_body_integrate_linear) stores `a = force * inv_mass` with the force's `w`. It
    zeroes each locked axis in `a` and in the velocity, and clamps `|a|` to +0xe4. The length goes on the
    FPU (`mula/madda/madd`, `sqrt`). The direction goes on the VU (`x+y`, then `+1·z`, `vrsqrt`), and
    unordered values clamp.
  - The position is `x = (x + v h) + ((a·½)·h)·h`, one `vmulx` at a time. The velocity is
    `v = damping·v + a h`, with `w` the damping's. Then `|v|` is clamped to +0xe0.
  - The angular half is skipped when the inverse inertia is bitwise (0,0,0,1). So (−0,0,0,1) is *not*
    pinned, and the test covers that case.
  - `alpha = inv_inertia·moment` (with the inverse inertia's `w`) gets the same axis locks and length clamp
    as the linear half. Then:
    - `q' = spin(q, rates)` (atv_quat_spin, the same as the biplane's `spin`);
    - `q'' = spin(q, alpha) − q·|q'|²` (atv_quat_spin_rate). Here `|q'|²` is taken on the FPU in the order
      x, y, z, w; MX Unleashed sums it on the VU as w+x, y, z;
    - `q = (q + q' h) + ((q''·½)·h)·h` over all four lanes, then the chassis' quaternion normalize.
  - The rates are `ang_damping·rates + alpha h`. Each rate is then held within ±max: `!(r <= m)` stores
    `m`, otherwise `r < −m` stores `−m`.
- **End of substep** (atv_body_end_substep) keeps each correction lane's peak by magnitude (`abs.s`
  compare, replacing when the new value is not ≤ the peak). It copies the force to the last force and
  clears the force and the correction to (0,0,0,1). It does the same for the twist peak, the moment and the
  twist. This is the same as the biplane's.

## The port

- `crates/game-sim/src/atv_body.rs` (new, `pub mod atv_body`) holds `Body` with every field the two calls
  read or write, `advance` (the linear half), `integrate` and `end_substep`. `view`/`set_view` map it to and
  from the `quad::Body` the listeners take: force, moment, quaternion, rates, velocity, and the correction
  peak (the PreProcess's `reset_each_frame`, +0xc0).
- `quad::frame(world, p, q, body, dt, table, m)` runs `biplane::World::step` over the quad alone:
  - preforce runs `ground_head(dt)`;
  - each substep runs force (`ground_step(h)`), then integrate and end of substep while `enabled`;
  - post runs `air_state(dt)`.

  Each listener sees the body through `view` and writes back through `set_view`.

## Proof

`cargo test -p game-sim --test atv_quad` (`./kit check -- -p game-sim --test atv_quad`): 16 passed.

- `the_quads_rigid_body_step_is_the_originals`: 10,000 generated bodies. Each runs the original's vt +0x54
  and then vt +0x40 (the slots are asserted) on the 0x230-byte body. Unread words are filled with random
  filler, the same filler both times. The run may write only the body and the stack, and the port's body
  must equal the original word for word. Paths, each seen ≥30 times: pinned, a linear axis locked, accel
  clamped, speed clamped, angular pinned, angular −0 not pinned, an angular axis locked, angular accel
  clamped, a rate held at the top, a rate held at the bottom.
- `the_quads_physics_frame_is_the_originals`: 3,000 of the existing generated states. Each also gets a
  generated body, whose six listener fields are the state's. The test lays out a world of the quad alone
  in RAM:
  - the list arrays;
  - a Force wrapper with the image's wrapper vtable;
  - the quad's three interface words, from the image's quad.

  It then runs atv_world_step with dt (a third of the time exactly 1/30; 1 to 5 substeps). Every Moto call
  target is hooked to log only; the node position hook writes as the PostProcess test's does. Every region
  (quad, body now 0x230, world, state, record, four wheels, player) and the call log must be equal. Paths
  covered: 1–5 substeps, quad off, body off, both on. The test also asserts that the image's world has 1
  iteration at 115, and that the image's quad has this wrapper and these interface vtables.
- Mutations, each reverted. These 10 turned both tests red:
  - the |q'|² order;
  - the q sum order;
  - dropping the correction clear;
  - dropping the twist clear;
  - swapping the damping operands;
  - the length sum order;
  - dropping the speed clamp;
  - dropping the angular-accel clamp;
  - dropping the accel clamp;
  - pinning by float compare.

  One more mutation did not compile.

  Two survived, and neither changes the result:
  - `r < −m` → `r <= −m`. At r == −m both store the same value.
  - The peak compare's NaN ordering. The generator makes no NaNs.

## Not done, filed

- The Moto that runs SLUS-21104's ported bike steps and atv_wheel on the quad's fields in place of each
  `Call`. These are the input limiter, clutch, preload, backflip, airborne control, suspension, tire forces
  and wheel integrate. Each needs an adapter from `Quad` to the bike step's types, proved on the quad.
- The calls with no SLUS-21104 port yet: engine update, rev limit, auto shift, head tail, gravity, spin
  air, air drag, peak rates, wheel clear, vehicle post, node position, and the contacts' collision search.
- The brief's proof: one whole frame off the image's own world, with only atv_ground_query hooked. This
  needs the two items above and MD76's ground query. It also needs a reader from the image's quad into
  `Quad`/`Params`/the body (the reverse of the test's `poke`), and a decision about the five other
  preforce listeners (the second ATV and four more), which run between the quad's own.

## Not verified / not 1:1

- No real frame of the game has been run through the port. The frame oracle runs the original's world
  step, integrate and listeners, but on a laid-out world of the quad alone, with every bike step hooked.
- The world step is MX Unleashed's (`biplane::World::step`). SLUS-21104's passes `h` to the constraints,
  and that is not modelled. The quad's world has no constraints.
- The other riders' preforce listeners are not part of `quad::frame`.
- The body's reset (vt +0x3c) and its force listeners (+0xe8) are not ported. Neither is called inside the
  frame.
- `atv_body::Body` and `quad::Body` are two views of one body. `frame` copies between them around each
  listener, and no ride uses `frame` yet.
- The PC exe's integrate was not found: its PhysicsBody vtable is named differently. So it was not read
  first (AGENT.md asks for that). The PS2 oracle covers every branch.
- Clippy was not run (it cannot lint game-sim; see memory). `atv_body.rs` uses only ps2-float ops.
