# 1 — save state tick one (PART)

Task S107. 2026-10-25.

## What I did

- **Ee-stepped first_recording's chassis frame against ride.rs.** I ran the original's bike vehicle update
  on `ps2_oracle::Ee` from the save state's RAM image, with a hook on every contributor that runs it as a
  nested call and logs the chassis' force (`+0x130`) and torque (`+0x140`) after it. I put matching
  prints in `Ride::frame_bumped`'s substep loop and compared the two. By HEAD the S64 brief's 1.3%
  pitch-acceleration parting was gone. The run parted on **one lane only**: `chassis.ang_hist.y` (and its
  `prev`), one ulp at tick 1 (`0xba288d91` vs `0xba288d90`). The torque agreed word for word at every
  contributor of both substeps; `ang_accel` did not. The cause was the **inverse inertia**. The original
  takes it with the general affine inverse when the `.phy` is loaded: the chassis vtable's `+0x7c`
  copies the tensor to `+0xb0` and inverts it into `+0xf0` (`./kit fn 0x00158e10`, then
  `./kit fn 0x001e7180`). The port used one reciprocal per axis. On `race125p` (diag 22.4, 18.4, 10.9)
  the y lane is `0x3d5e9bd3` there and `0x3d5e9bd4` here, and the off-diagonals are `−0`, not `+0`. A
  per-frame write check (a hookless Ee, `ee.written`) shows nothing in a frame writes `+0xf0`.
- **`math::inverse`** (crates/game-sim/src/math.rs): a port of the affine inverse and its adjugate, read
  line by line off `./kit dis 0x001e7180` and `./kit dis 0x001e6fd0`. The determinant and the cofactors
  run on the FPU (the accumulator's mula/msub/madd/suba, as `fpu::Acc`). `1/det` is `div.s`. The rows
  are scaled with VU0's chopping `vmulx`. Column 3 is read as (0, 0, 0, 1). It returns `None` on a zero
  determinant, where the original returns 0 and leaves `out` as it was.
- **`Ride::new`** builds `chassis.inv_inertia` through `inverse_inertia` (ride.rs): the tensor laid on
  a diagonal with `w` one and put through `math::inverse`. It falls back to the old reciprocals only on
  a singular tensor.
- **`SAVE_STATE_AGREES_THROUGH` 0 → `u64::MAX`** and **`SAVE_STATE_POSITION_AGREES_THROUGH` 41 →
  `u64::MAX`**. The S64 save-state run now agrees on every lane on every tick of
  `context/capture/s64-slow`. The ratchet no longer asks for a raise at `u64::MAX`.
- **S89's rides, tick 1** (the second half of the brief). I Ee-stepped the S89 images the same way
  (`along`, 3 frames). The Ee reproduces the capture's rows exactly (Ee tick 2's `vel.x` is row 0's), so
  the image is a good start. The port parted inside tick 1, for two reasons I fixed:
  - **The wrong bike.** The S89 test rode `race250a.phy`. The states' front wheel holds `race250p`'s grip
    table (`SideForce`/`SlippingSideForce` DropOff 1.5, Scale 41, SpeedScale 0.006 against `race250a`'s
    1.6/40/0.0055; the two files differ only in those tire curves). The test now rides `race250p.phy`
    and asserts the front's 20 surface-0 grip words against the image (wheel `+0x238 + 0x40·k`).
  - **Levers and engine unseeded.** The test now calls `seed_levers` and `seed_engine_state`, as S106's
    braked-slope test does. The image's clutch lever (`+0x65c`) is 1.0; the port's walked up from its
    default. On its own this did not move tick 1, but it is the image's state.

  With both fixes the first substep of tick 1 agrees word for word at every contributor. The second
  parts at the front wheel's tire forces: see below and S121.
- **S89 ratchets.** `S89_AGREES_THROUGH` [0, 1] → [1, 1] (`along` already agreed through its tick 1; the
  first compared tick of both rides is 2). In `S89_LANES_AGREE_THROUGH` I re-pinned the two lanes that
  are S120's red on bare HEAD to this run's values: front `slip.locked` 279 → 280, rear 277 → 276. They
  describe a different run now (the right bike, the levers seeded), so the old pins no longer measure
  anything. S120's question (what in main moved them on `race250a`) is left to S120; this re-pin makes
  s3's gate green on that lane.
- **Re-pinned hashes.** The new inverse inertia moves every ride's bits, so these were regenerated:
  `HUNDRED_TICKS` (src/lib.rs, tests/hash.rs, tests/platforms.rs); in tests/platforms.rs `HASHES`,
  `TRACE_DIGEST`, `VEL_Y`, `FINAL_DUMP_HEAD`, `FINAL_DUMP_DIGEST`, `FIXTURE_TEXT_DIGEST`,
  `RIDERS_HASHES`, `RIDERS_DIGEST` and `RIDERS_WIRE_DIGEST` (spliced const by const from
  `print_the_vectors`/`print_the_riders_vectors`); and tests/ride.rs's two MD25 folds. Only values
  moved: `FINAL_DUMP_LEAVES`, `EVERY_LEAF_*` and `RIDERS_WIRE_LEN` are unchanged, and so is `POS_Y`.
- **Oracle proof** `the_affine_inverse` (tests/math.rs), and an image check
  `the_chassis_inverse_inertia_is_the_save_states` (tests/replay.rs). research/oracle.txt names
  `mat4_affine_inverse`, `mat4_affine_adjugate` and `chassis_set_inertia`.

## What the original does

A rigid body's inertia setter (chassis vtable `+0x7c`, `./kit fn 0x00158e10`) stores the 4×4 tensor at
`+0xb0` and tail-calls the affine inverse (`./kit fn 0x001e7180`) with `out = +0xf0`. The inverse:

1. Takes the 3×3 determinant `m0·(m5·m10 − m6·m9) − m4·(m1·m10 − m2·m9) + m8·(m1·m6 − m2·m5)`. The two
   bracketed minors use the accumulator (`mula`, `msub`), the third is plain `mul`/`sub`, and the sum
   is `suba`/`madd`.
2. If it is 0, returns 0 with `out` untouched.
3. Otherwise calls the adjugate (`./kit fn 0x001e6fd0`) with flag 1, which stores the cofactors
   transposed. Translation cofactors use row 3; column 3 is taken as (0, 0, 0, 1). The negated
   cofactors are `neg.s` of an accumulator result, so a diagonal tensor's off-diagonal zeros are `−0`.
4. Computes `1/det` with `div.s`, loads it with `qmtc2`, and scales each of the four rows with
   `vmulx.xyzw`.

## How it was proved

- **Function oracle, 10,000 inputs**: tests/math.rs `the_affine_inverse` runs `mat4_affine_inverse`
  on the Ee against `math::inverse`, bit for bit, with the return value against `is_some()` and the
  untouched `out` on the zero determinant. The inputs are a third diagonal tensors with `w` one, a third
  affine matrices of numbers in ±10, and a third `Gen::mat`'s awkward quadwords (±0, extremes, denormals,
  Inf/NaN patterns). One in twenty has a repeated row; 947 of the 10,000 were singular. All agree.
- **RAM image**: tests/replay.rs `the_chassis_inverse_inertia_is_the_save_states`. A ride's built
  `inv_inertia` rows equal first_recording's chassis `+0xf0` word for word (tensor diagonal `41b33333
  41933333 412e6666`).
- **Save-state replay**: `the_save_state_run_starts_from_the_originals_wheels`. Every lane of S64's
  quarter-speed capture (chassis, both wheels, slip flags, position) agrees on every tick. On HEAD it
  parted at tick 1 on `ang_hist.y`.
- **S89**: `the_dallas_rail_rides_replay_from_their_save_states` asserts the bike's grip table against
  the image. The substep-1 agreement was shown by the temporary Ee trace described above (removed before
  the commit; the method is S64's journal's plus hooks on `wheel_surface_drag` and
  `wheel_spin_integrate`).

## Not verified / not 1:1

- **S89 still parts at its first compared tick (2).** It is now inside tick 1's second substep: the
  front wheel's swept contact on the bed is placed at z 2902.216 where the original's is 2902.207
  (`0x45356375` vs `0x45356353`; the port's equals its load point), and the compression (`0x3eec9c09`
  vs `0x3eec9a08`) and the spring force follow. I did not find how the original places a swept-only
  contact's point. That is S121. So S89's bit-exact acceptance is **not** reached, and
  `S89_AGREES_THROUGH` is only [1, 1].
- **The S89 lane re-pin is not S120's fix.** Front/rear `slip.locked` were re-pinned to this run (280,
  276). Why main moved them on the old `race250a` run is unexplained, and S120 stays open for that.
- **The singular-tensor fallback in `Ride::new`** (per-axis reciprocals) is the port's own. The
  original would leave `+0xf0` as it was, which for a fresh body is whatever the constructor put there
  (not read). No `.phy` on the disc has a zero inertia lane, so the fallback never runs.
- **The gate is red, but not on S107.** `./kit check --all` fails only on S119's
  `the_captured_backwards_turns_never_wreck` ("flat agrees through tick 213, S68_AGREES_THROUGH says
  330"), which S119 found on bare HEAD bb13cfa6 with the same 213. `./kit done` refuses a red gate, so
  the work is committed and pushed on task/S107 without a tick.
- **Cross-architecture**: `crates/game-sim/tests/cross/platforms.sh` passes `platforms` (the new
  vectors) on aarch64-unknown-linux-musl and wasm32-wasip1. Both fail `replay`: aarch64 on the same S119
  line, wasm32 with a message-less abort I did not chase to a test (wasm panics carry no text).
- **`seed_levers` on S89** is not proved to matter: it did not move tick 1. It is there because it is
  the image's state.

## Next

Rerun `./kit done S107 "<msg>" --skip-audit` once S119 has made main's gate green (the audit's only
leaks are other tasks' plan/DONE.md addresses). After that, S121 (the swept contact's point on the rail
bed), then raise `S89_AGREES_THROUGH`.
