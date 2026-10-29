# 1 — atv step (PART)

Task MD37b. 2026-10-29.

## What I did

- Read the brief's premise against the tree first. MD37b is to run the quad "on MD40's SLUS-21104 moto
  steps and frame driver", but MD40 did not port those: it ported the backflip and filed the rest as MD62
  (airborne control) and MD63 (the rest of the bike step); nothing drives an MX vs. ATV ride frame in
  game-sim yet, and Desert Oasis has no world/collision port. So the brief's test ("MD37a's captured ATV
  rides replay bit-exact") cannot be met in this task. I decided to port the pieces that are the quad's
  own and that stand alone, prove each against SLUS-21104 itself, and leave the frame for follow-ups,
  ending MD37b blocked after them so it comes back as the replay.
- `crates/game-sim/src/quad.rs` (new, `pub mod quad` in lib.rs): `Quad`/`Params`/`Wheel`/`Body`, and
  - `stand_upright` — the quad's replacement for the bike's stand-upright + balance;
  - `roll_pitch` — wheelie and stoppie, with the pitch integral and the stoppie latch;
  - `ground_pitch_torque` — the ground push under the quad's guard;
  - `rear_pair_flag` — the rear wheels' +0xb4 flag at the end of the Force body;
  - `Body::{add_force, add_moment, add_world_moment, add_local_force}` — the rigid body's vtable calls
    these four make, with its two quaternion rotates written out op for op.
- `crates/game-sim/tests/atv_quad.rs`: five tests off MD37a's `atv-r-spawn.ee.bin` (copied from s2's
  context/, git-ignored).
- research/oracle.txt: an `# MD37b` block (field map, the body calls) and rows atv_body_add_force,
  atv_body_local_moment, atv_body_local_force, atv_quat_axes.
- Filed MD64 (the Force body and frame: ground_head, the rest of ground_step, wheel_contacts, air_state,
  update tail, tire_phy) and MD65 (wreck on WreckATVm.col, put-back, net unpack, the unpaired PC slots).

## What the original does

Recipes: `./kit fn atv_quad_stand_upright`, `atv_quad_roll_pitch`, `atv_quad_ground_pitch_torque`,
`atv_quad_ground_step` (all `atv` rows in research/oracle.txt).

- **Force body order** (atv_quad_ground_step): bail if no vehicle state or it is 3/4; one moto call; if
  the +0xc80 word is clear: stand_upright, roll_pitch, three moto calls, the engine, two more, the ground
  push; then (either way) the four wheels' clear, contacts, suspension, tire forces, the rear-pair flag,
  four wheel integrates and two tail calls.
- **stand_upright**: only with all four down, the two balance words different, no gas and |speed| < 10.
  The mean of the four contact normals ((LF+RF)+LR)+RR × 0.25; gravity's slide along that ground
  (g − n·(n·g)); the body's forward axis flattened onto the ground and renormalised (rsqrt; NOTHING when
  nothing is left); the slide split into along/across that forward; the across part pushes back scaled by
  a 0..10 → 1..1 map of speed (a constant 1 as compiled, kept as the map); braking also cancels the along
  part. One add-force.
- **roll_pitch**: airborne does nothing. Wheelie: rear down, gas limited > 0, speed > 5, no rear brake,
  no clutch, fore-aft lean ≥ 0.5 → pitch command = max × lean fore-aft scaled by (1 − speed × 1/top), a PI
  on pitch with the integral's rate gain 500 (the bike's 1000), the moment into the body through the local
  moment call; its yaw term is multiplied by a literal 0 but the call is still made. Stoppie: starts with
  the front down, front brake, speed > 15, lean fore-aft ≤ −1; held until the rear comes down, the front
  brake lets go, the lean rises past −0.1 or speed falls to 7; its command is the [Stoppie] keys', yaw
  rate scaled by speed / top speed (zero top speed: no yaw term).
- **ground_pitch_torque**: rear down and the +0x12c4 gate non-zero, then the bike's push body: with the
  +0xdd8 push on, a local force (0, 0, gain × (target − speed)) through the local-to-world call.
- **rear_pair_flag**: (LF ‖ RF touching) && !LR && !RR → both rears' +0xb4 = 1, else both 0.

## How it was proved

Function oracle (`crates/game-sim/tests/atv_quad.rs`), the original run in ps2-oracle off
`context/mxvatv-ps2/md37a/atv-r-spawn.ee.bin` with the quad, body (vtable 0x003cd200), world and four
wheels laid out in RAM, compared word for word over every region, and the original may write nothing
else:

- stand_upright: 10,000 generated states, every path (off, no forward left, braked, rolling) ≥ 30;
- roll_pitch: 10,000 (airborne, wheelie, neither, stoppie starts / held / ends) ≥ 30 each;
- ground_pitch_torque: 10,000 (gated, no push, pushed) ≥ 100 each;
- rear_pair_flag: 2,000, run from the flag test to the first wheel integrate, all 16 touch patterns;
- the spawned quad's own object read through the field map (wheels touching, fronts steered, the
  [Wheelie]/[Stoppie] values, the world's gravity).

All pass bit-exact. A mutation check (pitch gain rate 501, stand-up speed 9) reds two of them, so the
compare bites. `math::quat_axes` matching the original's quaternion-to-axes routine is proved through
stand_upright.

## Not verified / not 1:1

- **No ride replay.** The brief's test, MD37a's captured ATV rides replayed bit-exact, is not done: the
  frame driver and the rest of SLUS-21104's moto step (MD62, MD63) and the rest of the quad's Force body
  (MD64) are not ported. MD37b ends blocked after MD62, MD63, MD64, MD65.
- **"Every bike replay hash unchanged"** holds trivially: nothing the bike runs calls quad.rs. It is only
  checked by `./kit check --all` staying green.
- Not ported: atv_quad_ground_head, the rest of atv_quad_ground_step, atv_quad_wheel_contacts (engine
  contact-list plumbing, not standalone), atv_quad_air_state, atv_quad_update's tail,
  atv_quad_tire_phy, the wreck on WreckATVm.col, put-back, net unpack, the unpaired PC slots (MD64,
  MD65).
- The PC exe's pc_quad_* functions were read for stand_upright, roll_pitch and ground_pitch_torque only
  (they match the PS2's clauses); no PC-side oracle.
- Field and parameter names (max pitch command, gains, push target/gain/gate, balance words) are guessed
  from the MX Unleashed analogs in move.rs, not from the .phy keys' readers.
- The oracle used only the atv_ (Race) image; atv_s_ runs the same code, but its image was not used.
- Generated states, not states from rides: the inputs cover the paths, not the distribution a ride has.

Nothing was taken from opentennis.

## Next

When MD62, MD63, MD64 and MD65 have landed, MD37b reopens as the replay: drive the quad with MD64's Force
body on MD63's step and replay MD37a's 15 csvs (s2's context/capture/md37a) bit-exact.
