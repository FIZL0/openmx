# 1 — native sim (FINAL)

Task F15. 2026-11-02.

## What I did

- **ps2-float `native`** (`crates/ps2-float/Cargo.toml`, `lib.rs` "The `native` feature", `NATIVE` const). It is
  off by default. When on:
  - `fpu::{add, sub, mul}` and `vu::{add, sub, mul}` are `a + b` / `a * b`. madd/msub, the accumulator forms
    (`adda`, `mula`, `madda`, …), `Vf::{add, sub, mul, madd, msub, mul_q, add_q, opmula, opmsub}`, `Lane`'s,
    and the `F`/`Lane` operator impls are `#[inline]` through `cfg_attr(feature = "native", inline)`, so the
    exact build's codegen is unchanged.
  - `madd` is a product and then a sum: two roundings to nearest, not an FMA.
  - `sqrt` is the host's root of `|x|`. The crate is `no_std` only without `native`, because core has no
    `f32::sqrt`.
  - `div` is the host's divide, but `x / 0` still gives `±max`, and `rsqrt` is `div(a, sqrt(b))`. I kept
    these two guards on purpose. The ported code relies on `x/0 = MAX` (`MAX·0 = 0`), and an IEEE infinity
    turns that into a NaN that never leaves the ride. They are not "plain IEEE" in this one respect.
  - Kept as they are: the game's `libm` (`sinf`, `atan2f`, …, which now runs on native adds inside), the
    `to_i32`/`ftoi` conversions, `min`/`max`, and the VU MAC/clip flag model.
- **openmx `native-sim`** = `["ps2-float/native"]` (`crates/openmx/Cargo.toml`).
  - `build.rs` reads `CARGO_FEATURE_NATIVE_SIM`. It emits `OPENMX_SIM_TAG` (` · native sim` or empty) and
    `OPENMX_BUILD_ID` = `sim_id(id(..), native)`, which is the old id plus `, native sim`.
  - `web.rs` has a compile-time assert that `ps2_float::NATIVE == cfg!(feature = "native-sim")`. Without it,
    `native` unified in from somewhere else would ride native while the label and the handshake say exact.
- **Telling builds apart**:
  - `menu::BUILD_CORNER` (the main menu's corner) ends ` · native sim`.
  - The new `menu::VERSION_LINE` (`openmx v0.9.5 · a772b5e · native sim`) is what `--version`/`-V` prints.
    A native build also prints it as the very first line of every run.
  - The iPhone log's first line carries the tag.
- **Netplay**: the handshake already refuses a peer whose build id differs (`net.rs` `version_differs`; the
  lobby list greys it as "another version"). Because the sim mode is now in the id, a native build refuses an
  exact one either way, release or dev, same commit or not. Two native builds of one commit meet as before.
- **`tools/build.sh linux --native-sim`** (and `windows`/`all`) passes `--features native-sim` into the sniper
  container's cargo. The concept build is `dist/linux/` in slot s1 (`openmx v0.9.5 · a772b5e · native sim`,
  GLIBC_2.30). It is not published anywhere; the release and beta pipelines are untouched.
- Tests:
  - `tests/build_stamp.rs` `a_native_sim_builds_id_is_not_an_exact_ones`.
  - `tests/menu.rs` `the_version_line_says_which_simulation`.
  - The two existing stamp/corner asserts now include the tag, so they hold on both builds.

I checked opentennis first: it has no such feature, so there was nothing to take.

## Measurements

All of these are on this desktop. Headless free06, `--frames 1500 --timings`, riders on the default
self-ride. The machine was shared with other agents (load 2–13), so I give ranges over 2–3 runs and take the
quieter run as the figure. "sim" is the session's mean ms a frame.

| build | 1P sim | 2P sim | frames over 16.7 (2P) |
|---|---|---|---|
| exact (main as is) | 4.4–4.7 | 7.6 (11–13.5 under load) | 0–3 |
| exact, every op and helper `#[inline]` | 4.2 | 7.4 | 0 |
| native | 2.2–3.0 | 4.3–5.1 | 0 |
| ridden tape (`first_recording.p2m2`, 1P): exact / native | 1.38 / 0.65 | | 0 / 0 |

- Native costs 0.55–0.65 of exact here, and 0.47 on the tape. The brief's Deck numbers (2P 6.1 → 2.84 ms,
  1497 → 65 frames over budget) used the same swap without the inlining.
- Inlining the exact path saves only ~5%, with the ride hash unchanged (`0x4501646f3f6ff996`). The time goes
  into the f64 two-sum and the chop/align branches, not into the call. So a cheaper exact path has to be
  SIMD: four lanes of f64 two-sum at once.

### Which verdicts move (`cargo test -p game-sim --features ps2-float/native`)

191 of 1028 game-sim tests fail and 837 pass. Many of the passes are tests that skip without their captures
in this worktree's `context/`, so 191 is a floor.

- Most failures are bit-level:
  - wheel mounts and the attachment speed are 1 ulp off;
  - an airborne control angle is 9 ulp off;
  - the plane's first-tick velocity is ~470 ulp off on a 7e-7 lane (2303.7039 vs 2303.7048 ft after one
    tick);
  - a landing tick is off by 6/33/108 ulp in quat/vel/spin;
  - inverse inertia is 2 ulp off.
- The bigger ones, with want vs got:
  - `idle_roll_offs_on_free06…`: the bike strays 0.198 ft from the original (limit 0.01).
  - `a_free09_trunk_wrecks…`: the velocity after the hit is 2.40 ft/s off.
  - `the_rustic_windmill…`: the apex velocity at tick 24 is 11.2215 against 11.2484 ft/s (0.24%).
  - `the_captured_idle_roll_back_never_wrecks`: agrees through tick 118 of 699.
  - `the_biplane_acceptance_run`: parts at tick 1 (it used to agree through 1526).
  - truck and path-car hits part at ticks 1–2.
  - `md25_mx_vs_atv_rides_as_before`: the ride hash changes.
- The worst-hit areas by count: replay.rs (21), ragdoll_wrecks (10), ragdoll (8), platforms (7), car_drive (7),
  car_tire (6), traffic, rider_anim, math (5 each).
- I did not run openmx's own suite with native. Its replay and tape asserts compare ride hashes, which change
  by construction.

### Does it feel the same (a ridden tape)

`first_recording.p2m2` (the human's 60 s freeride), played open-loop through both builds:

- Speeds match to 0.1 mph for 4 s, drift by 1–4 mph from 5 to 13 s, and part at 14 s. One build loses the line
  where the other keeps it.
- After that they are two different rides. The exact build has 3 wrecks and the native 4. They end 1100 ft
  apart.

That is what any perturbation does to an open-loop tape, and it says nothing about feel. A tape replays the
inputs, not the rider's corrections. Feel has to be judged by a person riding it.

### What keeping both modes costs

- **Netplay**: done here, through the build id. The cost is a split player base. Lobbies of the other mode
  show as "another version", and rollback and desync checks only hold within one mode.
- **Proofs**: every bit-exact test is valid only for the exact build. The gate keeps running exact only. The
  native build has no regression net beyond "it runs" unless someone writes tolerance tests for it.
- **Code**: about 70 lines in ps2-float, all behind one const or cfg. No sim code changed. The cost grows if
  native ever needs its own fixes, such as a sim branch that relies on the PS2's chop.
- **Shared replays and tapes**: a `--record` tape made on one mode replays differently on the other, as above.
- **The alternative on main**: a cheaper exact path. Inlining is not enough (−5%). The candidate is
  4-lane f64 SIMD for `Vf` add, mul and madd: two-sum and the chop as vector ops with a lane mask for the
  step-back. It needs its own Deck number. Filed as a follow-up.

## How it was proved

- The exact build is unchanged. ps2-float's 61 tests are green, and openmx's stamp and corner tests are green
  in both modes. The free06 ride hash is the same before and after the inline experiment.
- The native build's label and id are covered by `build_stamp.rs` and `menu.rs`, run with
  `--features native-sim`.
- `dist/linux/openmx --version` and the first line of a headless run both say `native sim`.
- The timings and the test outcomes above come from the runs listed. Raw outputs were in this session's
  scratchpad and are not kept.

## Not verified / not 1:1

- **No Deck numbers from this session.** `ssh deck` timed out because the Deck was asleep or off the
  tailnet. The only Deck figures are the brief's (2P 6.1 → 2.84 ms).
- **Feel is not judged.** That needs a person riding the concept build. The tape comparison above cannot
  answer it.
- **Native is not plain IEEE everywhere.** `x/0` and `rsqrt(·,0)` still saturate, `sqrt` takes `|x|`, the
  game's libm is still the game's, and the VU flag model is unchanged.
- **Native leaves infinities possible.** Overflow on add/mul is no longer clamped to `±max`, and denormals
  are not flushed. I saw no NaN in the runs above, but nothing guards against one.
- **The Windows native build is untested.** It is only the flag passed through.
- **openmx's own test suite was not run in native mode** (see above).

## Next

- A person rides `dist/linux/` (or a fresh `tools/build.sh linux --native-sim`) on the Deck and another
  device and says whether it feels the same. Time it there with
  `--headless --track free06 --players 1|2 --frames 1500 --timings`, exact against native.
- F15's follow-up: the SIMD exact path and its Deck number.
