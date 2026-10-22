# 1 — the curve table in double (FINAL)

Task S104. 2026-10-22.

## What I did

- **Found the input.** S94's biplane run parted at tick 262: `body.rate[0]` and the autopilot's
  `elevator_command` were each 1 ulp off. A temporary probe in `tests/replay.rs` ran the port to tick
  261. It wrote the port's `Flight` over the plane in the save state's RAM: the body, the pad, the
  surfaces, the integral, the measure, gravity, the crash test, the aero true speed, the engine and
  the three tires. Then it stepped the original's `dynamics_world_step` once,
  with dt 0.03333, the collider resolve hooked to a no-op (the port has no contacts at 261/262), and
  the autopilot (0x352460) hooked to log its inputs on every substep.
  - The seeded original reproduced the capture's elevator command, -0.50129163. The port gave
    -0.5012917. So the difference is inside the step, not in the state carried across ticks.
  - On substep 4 every input agreed: elevator 0, rate.x -0.25996727, roll -0.6546927, nose_y
    0.0548653, speed 85.42796, integral -0.5234204. The port's `autopilot` on these inputs gave
    -0.5012917 with the port's `CurveTable` and -0.50129163 with the RAM's table.
  - **The cause is the `{cos, sin}` table.** The autopilot's turn term reads it (`curve(roll)`).
    `CurveTable::build` used `libm::cosf/sinf`, and 3968 cos and 4150 sin entries of the 6000 were one
    ulp larger in magnitude than the game's. Entry 0, for example, is `bf7fffff` in RAM and `bf800000`
    in the port.
- **The game's builder** (0x12e260) works per entry. It takes the angle `(float)i * step - pi` in
  single precision, widens it to double (0x417b48), and calls the SDK's double `cos` (0x409a50) and
  `sin` (0x40a298). Those are fdlibm 5.3: `__kernel_cos` 0x407980, `__kernel_sin` 0x4087a8,
  `__ieee754_rem_pio2` 0x406278. It then narrows by chopping (0x418700). This is the same pair of
  conversions as the arc cosine builder (0x12e150), which `air.rs` already ports as `widen`/`narrow`.
- **Ported** `air::cos_sin`: fdlibm's `cos` and `sin` in f64, with their kernels and `rem_pio2`. The
  constants are u64 bits, in the same style as the existing acos port. `rem_pio2` covers the
  one-step case (|x| < 3pi/4) and the medium case only as far as the table's angles need, which is
  n = 2 (it asserts this). `CurveTable::build` now uses `narrow(cos/sin(widen(angle)))`. Its docs,
  the camera module's docs and `ragdoll::Tables::build`'s doc no longer say the table is approximate.
- **Results:**
  - `tests/camera.rs`: `the_curve_table_is_the_games_own_to_a_rounding` (tolerance 1e-6) became
    `the_curve_table_is_the_games_own`. All 6001 entries now match the game's builder under the
    oracle bit for bit. A forced panic confirmed that it ran (91.7M instructions, 6001 checked) and
    did not skip.
  - **The biplane run now agrees through tick 691** (it was 261). `BIPLANE_AGREES_THROUGH` = 691.
    It parts at 692, the plane's first crash: C7's `crash` goes 0→1 and the throttle drops to 0
    there. On that tick the port's `plane.engine.power` is 0, while the capture keeps 250 until the
    respawn at about 775. I filed this as S105.
- **Consequences:** the bike's ride reads the same table, so the pinned vectors moved, but only
  their values. In `tests/platforms.rs`: HASHES, TRACE_DIGEST, POS_Y, VEL_Y, FINAL_DUMP_HEAD,
  FINAL_DUMP_DIGEST, FIXTURE_TEXT_DIGEST, and RIDERS_HASHES, RIDERS_DIGEST, RIDERS_WIRE_DIGEST.
  HUNDRED_TICKS changed in all three of its places. FINAL_DUMP_LEAVES, EVERY_LEAF_HASH and
  RIDERS_WIRE_LEN did not move. I regenerated the vectors with the two generators and spliced them
  constant by constant.
- I took nothing from opentennis. It has no table built like this one.
- The seeded-step probe was temporary, and I removed it. The proof that stays is the table test and
  the ratchet.

## How it was proved

- The full curve table is equal to the bit with the game's own builder running in the interpreter
  (`the_curve_table_is_the_games_own`).
- `the_biplane_acceptance_run` agrees through tick 691. I raised the ratchet past the data for one
  run to read off the parting tick, then set it to 691.
- `./kit check --no-fail-fast` passed except `net::udp::tests::two_builds_since_the_id_both_say_why`
  ("Address already in use", the known port race under parallel slots). That binary passed when
  rerun alone.

## Not verified / not 1:1

- **fdlibm's constants** were written from fdlibm 5.3's published values. I did not compare them
  word by word against the ELF's constant pools. Only the all-6000-entry equality proves them, and
  only on the table's angles.
- **`rem_pio2` is partial.** It is ported for the one-step and medium cases, and it asserts n = 2 in
  the medium case. Huge arguments (the `__kernel_rem_pio2` path) and other quadrants of the medium
  case are not ported and not tested. `cos_sin` is `pub(crate)` and has no caller but the table.
- **The regenerated platforms vectors** were produced on x86_64 only. The cross-architecture run
  (`tests/cross/platforms.sh`) was not done.
- **Bike acceptance runs:** not re-examined beyond the gate passing (they parted at tick 1 before).
- **Tick 692** (S105) is not investigated past reading the capture's columns.
