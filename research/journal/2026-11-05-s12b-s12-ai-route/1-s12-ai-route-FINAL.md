# 1 — s12 ai route (FINAL)

Task S12b. 2026-11-05.

## What I did

- **Found the controller.** `ai_update_mx` (0x00386d30) and `ai_update_vehicle` (0x002858d0), which the
  brief names, are not what turns the route into controls. They are the feeler fan and the avoidance
  scan, which is S12c. They write a steer direction and a speed for the route follower to take in.
  The per-tick throttle, brake and steer come from MXVehicleAI vt+0x90, slot 0x0045c530, at
  0x00385af0. That dispatcher zeroes the outputs unless (+0xd4)&2 is set. Interface types 4 and 5 (the
  fliers) have their own branches. Every other type, the bikes included, calls **0x00283ef0**, the
  base VehicleAI ground controller. All three free09 AI riders in the recording2 image have interface
  type 1, so they take that path.
- **Ported the controller bit-exact** as `ai::controls(&Tables, &Situation, &Tuning) -> Controls` in
  `crates/game-sim/src/ai.rs`. It produces the steer, pitch, throttle and brake, and the working values
  it leaves behind (heading, aim, roll, the flattened forward axis, and the target speed after its
  minimum).
- **Proved it** in `crates/game-sim/tests/ai.rs` (module `controls`):
  - `the_controller_matches_over_ten_thousand_ticks`: 10,000 random situations and tunings are written
    into the image's first AI rider, then run through the original's 0x00283ef0. All 12 output words
    match bit for bit. Each branch is hit more than 100 times: air 3212, landing 4252, wide-open
    throttle about 6400, graded brake, cruise, full lock.
  - `the_free09_riders_controls_are_the_originals`: the three AI riders exactly as the image holds
    them. The port, run on what each controller read, gives what that controller left in RAM: steer
    0.9975 / −0.8776 / −0.0904, throttle 1, brake 0, pitch 0 (all three are on the ground). Running the
    original again on the image gives the same values.
- **Mutants.**
  - The shared `CurveTable::sine` multiply order, `sin + (step·cos)·frac`, fails 43 of 10,000 cases.
    The controller's inline sine is `sin + step·(cos·frac)`, so the port has its own.
  - Three mutants pass, and on these inputs each one gives the same result as the real code:
    - one wrap instead of two before the table look-up. The second wrap only changes anything past
      about 1e7 rad, and the test already feeds landing angles up to ±3e5.
    - mul+add instead of `mula`/`madd` in the landing pitch filter. The two differ only on signed
      zeros.
    - no clamp before the arc cosine. The argument is a cosine over its own norm, so it never leaves
      −1..1.
- **Split the rest of the brief** into S279, S280 and S281:
  - S279 (part 1): the route follower and the track's AI path. This is where +0x1d0 and +0x1e0 come
    from.
  - S280 (part 2): the AIChrctr.aip reader. This is the skill's effect, i.e. where `Tuning` comes from.
  - S281 (part 3): the AI tick's order, the hand-off of the controls to the bike, and the free09
    opponent capture.
- Named these in `research/oracle.txt`: `ai_controls`, `ai_controls_dispatch`, `ai_update`,
  `ai_route_follow`, `ai_feed`, `ai_character_read`, `ai_rsqrt`, `ai_acos`, `curve_table_step`. I also
  added a note to `ai_tick_flag`: it is MXVehicleAI's route follower too.

## What the original does

The controller runs on `ai` (one MXVehicleAI object). Offsets:

| Field | Offset |
|---|---|
| up | +0xa0 |
| forward | +0xb0 |
| height | +0xc4 |
| spin | +0x70 |
| speed | +0x80 |
| flags | +0x30 (64-bit) |
| target direction | +0x1d0 |
| target speed | +0x1e0 |
| minimum speed | +0x254 |
| ground height | +0x204 |
| ground normal | +0x2c0 |

1. The target speed is raised to the minimum speed if it is below it.
2. p = forward with y = 0, normalised the VU way (`vmul`, `vadday.x`, `vmaddz.x`, `vrsqrt`, `vmulq.xyz`;
   `w` kept). The result is stored at +0x270. side = (p.z, 0, −p.x).
3. Three signed angles, each `acos(along · rsqrt(across² + along²))` and negated when across < 0. The
   dot products run on the FPU accumulator in x, y, z order.
   - heading = +0x208 (SteeringGain) × angle(target·side, target·p). Stored at +0x1f4.
   - aim = (−+0x2d0) × angle((target−forward)·up, (target−forward)·forward). Stored at +0x1f0.
   - roll = +0x208 × angle(up·side, up.y). Stored at +0x1f8.
4. steer = sine(clamp±π/2(heading)). If flag 0x10000 is set, +0x21c × spin.y is subtracted from
   heading first. The clamp tests `< −π/2` first.
5. pitch (+0xf4):
   - 0 when flag 0x20 (on the ground) is set.
   - Higher than +0x2b4 (PitchControlDistance) above the ground: sine(clamp±π/2(+0x21c × spin.x − aim)).
   - Otherwise:
     `clamp±1(mula(old pitch, 1−k) + madd(k, +0x2d4 × −(normal · (forward·cos L + up·sin L))))`,
     with k = +0x2d8 and L = +0x2ec (LandingPitchAngle).
6. steer is multiplied by +0x100 and pitch by +0x104, then both are clamped to ±1.
7. Throttle and brake, with t = target speed and v = speed:

   | Case | Throttle (+0xe0) | Brake (+0xe4) |
   |---|---|---|
   | v < t − +0x260 | 1 | 0 |
   | v > t + +0x264 | 0 | 1, or `1 − +0x334·(v−t)` under flag 0x20000000 |
   | otherwise | 0, or `mula(t, +0x32c) + madd(+0x330, t−v)` under flag 0x10000000 | 0 |

   Both are then clamped to 0..1.
8. The function returns 1.

The sine and cosine are inline:
- wrap twice with `a − 2π·trunc(a·(1/2π))`, then subtract 2π if the result is not ≤ π, or add 2π if it
  is < −π.
- index = (π + a) · 954.77045.
- sin = entry sin + 0.0010473721·(cos·frac).
- cos = entry cos − 0.0010473721·(sin·frac).

The step and index scale sit beside the table at 0x0046a690 and 0x0046a698, and the test asserts they
are the port's. The arc cosine (0x002849d0) clamps twice, then reads the fine table that
`ragdoll::FineAcos` already ports, the same way ragdoll's axis angle does.

Around the controller, MXVehicleAI::Update (0x00383000) runs, in mode +0xd0 == 5:
1. `ai_feed` (interface vt+0x8), which copies the bike into the AI.
2. The route follower (0x00383580).
3. vt+0x90 (this controller).
4. The type-1 climb rule. While +0x390 > 0, throttle is set to 0 and +0x390 counts down. Otherwise,
   when forward.y > +0x394 and speed < +0x30c, pitch is set to 1 and +0x398 counts up; after 5 s,
   +0x390 = 5.
5. Interface vt+0xc (0x0027fe90), which takes the controls to the bike.

These steps are S281.

Where the tuning comes from (S280): `ai_character_read` (0x00384f00) reads AIChrctr.aip, section
`AI_%d` keyed by the rider's skill (interface +0x104 = bike+0x11d4). Each Min/Max pair is lerped as
`max·t + min·(1−t)`, where t comes from the event's difficulty byte +0x117:
- d ≤ 100: t = (d−70)·0.0333333
- otherwise: t = (d−100)·0.04

The key → offset table is in my notes for S280: SteeringGain +0x208, OversteerCorrectionGain +0x21c,
PitchGain +0x2d4, PitchJitterDeadZone +0x2d8, PitchControlDistance +0x2b4, LandingPitchAngle +0x2ec
(×0.017453292), SpeedMin/MaxTolerance +0x260/+0x264, RollGain +0x2dc, RollControlDistance +0x2b0, and
the look-ahead, offset and collision keys.

## How it was proved

`./kit check -- -p game-sim --test ai`: 3 tests pass. The two new tests run on
`context/states/recording2.p2m2_SaveState.eeMemory.bin` and use the image's own curve table. Without
the image they skip.

## Not verified / not 1:1

- **The route follower is not ported** (S279), so the target direction and target speed the controller
  takes are inputs, not computed. Reason: it is 5936 bytes plus the path module and the .res path
  data. That is its own task.
- **The skill's effect is not ported** (S280). `Tuning` is filled by the caller; the AIChrctr.aip
  reader is not ported. Reason: it is a separate file reader with ~40 keys.
- **No free09 opponent capture** (S281). The controller is proved on random inputs and on one image's
  three riders, but not tick by tick over a ride. Reason: that needs the AI tick around it (feed, route,
  hand-off) ported first. Without them a capture can only re-prove what the image already shows.
- **The +0x2d0 gain's name is a guess** ("aim gain"). I did not find its aip key among the ones I
  read, so I don't know where its value comes from.
- **The fliers' controller branches** (interface types 4 and 5 in the dispatcher) are not ported. The
  bikes do not take them.
- **Three mutants pass because they don't change the result on these inputs** (see above). The second
  wrap and the arc cosine's clamp cannot be told apart for any input the game can produce. The fused
  filter differs only on signed zeros, which the edge picks include but which may never meet in the
  right combination.
- The fine arc cosine is the port's `Tables::build()`, not the image's. `tests/ragdoll.rs` proves it
  equal to the game's on another state; I did not compare it again on this image.

## Next

S279 (route follower), then S280 (the character reader), then S281 (the tick and the capture). S12c
(avoidance) writes inputs that the route follower reads, so S279 should leave a seam for it.
