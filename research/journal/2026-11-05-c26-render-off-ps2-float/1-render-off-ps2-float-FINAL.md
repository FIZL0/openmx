# 1 — render off ps2 float (FINAL)

Task C26. 2026-11-05.

## What I did

The brief said "23 files". On this branch only **11** of game-render's files use `ps2_float` (skin,
anim, vehicle, rider, creature, animated_toy, biplane, psg, toy, chassis, eco), and `gpu/` uses none.
I read every use and sorted it:

**Moved to host f32. These are per-frame draw paths that nothing reads back.**

- `animated_toy::palette`: each animated toy's joints at the picture's moment between two ticks. It
  ran the sim's `toys::pose` (the PS2-arithmetic key decode, slerp, `quat_to_mat4`) on a copy of the
  hints every drawn frame. It now uses skin's host-f32 decode, slerp and rotation rows over the same
  keys, with a hint-free binary search (`shown_rotation`, `shown_position`), and has the same edge
  behaviour: one or two keys blend first and last; key 1 is held before the start and the last key
  after the end; a position past the last key carries on the last slope. The sim's own toy pose (the
  collision that turns with it, hashed and sent) is untouched.
- `chassis::wrecked`: the wrecked bike's drawn axes. They came from `game_sim::math::quat_axes` in the
  PS2's arithmetic, and now come from a local host-f32 `quat_axes`. It is read by the drawn bike, the
  cut test, the bike particles and the skin. None of these feeds the sim.

**Left on ps2_float, and why:**

- skin.rs: `Clip::new(F(..))` is the sim's own timing record (`motion::Clip`). `impl motion::Joints`
  is how the sim reads the live pose. `Skeleton::nodes` is the snapshot the restart cross-fade fades
  from, which V15 proved bit-exact with `mat4_to_quat`.
- anim.rs and vehicle.rs: `Set::frame`/`driven`, `Stance`, and the chassis' `vehicle_anim` machine run
  once a tick, inside the online ticks (rolled back). `rewind_drawn` writes the motion `Snapshot`.
  The bike's pose feeds `ride_holds`, which become the sim's `rider.holds`.
- rider.rs: `ragdoll_rig` and `ragdoll_posed` build the ragdoll's rig and posed skeleton, and the sim's
  wreck is thrown from them.
- creature.rs: the trigger boxes (`segment_touches_box`, `probe_strikes_box`) are oracle-tested
  (`tests/trigger_box.rs`, `tests/trigger_contact.rs`). The trigger timers and `rand()` rolls are
  stepped by sim ticks.
- psg.rs: `world_vu` and `place_in` are the wheel mounts (S22), which are sim numbers.
- toy.rs: `Traffic::sim` and `physics` build the sim's cars.
- eco.rs: `ground_under` is the original's point query, and the plant reader cuts its result to
  65536ths of the extent. `tests/ecosystem.rs` compares every stood plant to the original's RAM word.
  It runs once at load, so it costs no frame time.
- biplane.rs: `between` already does its arithmetic in host f32 on `.0`. The `F` there is only the
  field type of the sim's `biplane_anim::Surfaces`.
- `chassis::placed`, `animated_toy::{toys, motion, rig}`, and the `F(…)`/`Vf::new` around them are
  type seams into sim APIs. No arithmetic happens there.

The openmx crate's draw side also holds Vf/F (car_draw, pace, chase, particles, tire_tracks…). That is
outside this crate, so I filed it as V124 (low). Opentennis has no equivalent change to take from.

## How it was proved

- `chassis::tests::the_drawn_axes_are_the_sims`: compares the host axes with `game_sim::math::quat_axes`
  over 10,000 generated unit quaternions. They agree within 1e-6.
- `animated_toy::tests::the_drawn_pose_is_the_sims`: compares the new palette with the old one (the
  sim's `toys::pose` with its hint, composed the way the picture always composed it). It runs 20
  generated clips, of both key kinds and 1–12 keys, on a five-joint chain with one unmapped joint, at
  2,000 times each from 0.2 s before the first key to 0.2 s past the last (40,000 poses). They agree
  within 2e-5 of each row's size. The worst seen was a translation row 3e-5 off at magnitude ~10,
  which is the parents' rotation ULPs multiplied by their reach.
- The sim is unchanged. I built release `openmx` from HEAD (a scratch worktree) and from this branch,
  in both exact and `native-sim`, and rode headless free09 for 600 frames with `--fps`. The ride hash
  was the same before and after in each build: exact 0x7664df74eed44401, native 0x90f1d1435be495b8.
- Frame time, from 3 interleaved rounds each, OPENMX_THREADS=1, on a shared machine:
  exact before sim 0.83/0.67/0.66, scene 0.14/0.10/0.10, draw 1.40/0.95/0.97 ms;
  exact after sim 0.81/0.82/0.63, scene 0.15/0.14/0.09, draw 1.33/1.36/0.86 ms;
  native before sim 0.32/0.36/0.31, scene 0.10/0.11/0.09, draw 1.15/1.30/0.99 ms;
  native after sim 0.38/0.28/0.27, scene 0.13/0.09/0.08, draw 1.33/1.03/0.91 ms.
  The difference is within the machine's noise. A microbenchmark of one five-joint toy pose
  (cargo test --release, a temporary timing loop that is not committed) gives:
  exact 2.1–3.5 µs before and 0.28–0.42 µs after (~8×);
  native 0.57–0.74 µs before and the same 0.28–0.43 µs after (~2×).
  On free09 that saves about 15 µs a frame across its five windmills in the exact build.

## Not verified / not 1:1

- **The brief's "23 files" was not found.** The branch has 11 files with ps2_float uses. Either the
  count included something else (the openmx crate's draw side, now V124), or files have changed since
  it was written.
- **The drawn toy pose and the wrecked bike's axes are no longer the PS2's bits.** They are within
  2e-5 of a row's size and within 1e-6 respectively. This is a rendering difference, which AGENT.md
  allows. The sim's own toy pose and wreck are untouched (the ride hashes are the same).
- **The toy palette's tie-breaking can differ from the sim's.** The sim walks from its hint, and a
  time exactly on a key can then pick the other pair, which gives the same quaternion within ULPs.
  Wide keys decode through skin's `turn & 0xfff` table, while the sim's `wide_at` reads past 4096
  into the narrow table. I did not check whether any disc key has a turn past 4095.
- **I did not run a pose shot against the disc's toys with the new palette.** I did not check which of
  the gate's shot/pose tests, if any, draw an animated toy. They pass unchanged. The generated-clip test
  is the proof, and free09's actual windmills were not compared frame by frame.
- **No frame-time gain could be seen above the noise.** The two paths moved are a few microseconds
  of a ~3 ms frame. The paths that cost real time in F (animation, ragdoll, triggers) feed the sim,
  so they stay as they are.

## Next

Nothing for C26. V124 covers the openmx crate's draw side.
