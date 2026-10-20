# 1 — s31 the humans backflips (FINAL)

Task S31. 2026-10-20.

## What I did

- **Captured the backflips myself rather than waiting on C6.** C6's `recording2-freeride.csv` (slot s4)
  has the flip state but not the stick, hold, contact speed or chassis, so I extended C6's spec and
  played recording2 back once more:
  - The spec is C6's `r2start.toml` plus 17 `s31_*` fields on the bike object `R` (`@game_manager -> 0x34
    -> 0x34 -> 0x8 -> 0x8 -> 0x0`). The fields:
    - stick `R+0x620` (f32x4; `.z` is x, `.w` is y) and hold `R+0x630`;
    - brake `R+0x640`, contact velocity `R+0x430`, authority `R+0x4e8` (`.y` is the ramp time);
    - tuning `R+0x500` and `R+0x510`, wreck angles `R+0x770`;
    - touching `R+0x7d0 -> 0x40`;
    - on the chassis `R+0x7a4`: velocity `0x260`, body velocity `0x270`, Euler angles `0x90`, world and
      body angular velocity `0x280` and `0x290`, and speed `0x34`.
  - The spec is kept beside the capture as `context/capture/acceptance/recording2-backflips.toml`, and
    the playback scripts are in `context/capture/s31/recipe/`. They are C6's AT-SPI recipe: Reset,
    arm the recorder, then Tools/Input Recording/Play on a *copy* of the .p2m2.
  - Result: `context/capture/acceptance/recording2-backflips.csv`, frame counters 2374..4267, 1894
    rows, no gaps. Every flip and rider column is identical to C6's capture.
- **Measured the three backflips and one gesture that never fired.** The full index follows; C6 can
  take it as is.

  | | flip 1 | flip 2 | flip 3 | (armed only) |
  |---|---|---|---|---|
  | lip open (end-of-tick rows) | 2493–2510, **18 ticks** | 3281–3299, **19 ticks** | 4065–4084, **20 ticks** | open from ≤3512 |
  | stick past −0.8 | 2499 | 3283 | 4068 | 3517 |
  | arms | 2500 | 3284 | 4069 | 3519 |
  | stick crossing, last below −0.8 → first above +0.8 | 2504 −0.877 → 2505 +1.0: **1 tick, travel 1.88** | 3288 −1.0 → 3289 −0.328 → 3290 +0.843: **2 ticks, 1.84** | 4074 −1.0 → 4075 +0.574 → 4076 +1.0: **2 ticks, 2.0** | — |
  | fires | **2506** (substep 1: grace 0.583) | **3292** (substep 1) | **4077** (substep 1) | never: airborne at 3522 with the stick still back |
  | hold at fire, torque | 469.4 → −1400 (clamped) | 32.33 → −1215.55 | 54.05 → −1360.34 | — |
  | takeoff (airborne) | 2511 | 3300 | 4085 | 3522 |
  | turning | 2512–2579, 68 ticks | 3301–3378, 78 ticks | 4086–4123, 38 ticks | — |
  | rotation over (`flipping` down, torque decayed below 800) | 2580 at −786.35 | 3379 at −782.26 | never, still turning at the wreck | — |
  | body pitch rate, peak | −3.111 rad/s | −2.761 rad/s | −3.111 rad/s | — |
  | rotation, takeoff to down (Σ body rate × dt) | **−1.043 turns** | **−1.000 turns** | **−0.600 turns** | — |
  | down | lands 2595, no wreck | lands 3417, no wreck | **wreck** reason 6 at 4124, mid-flip | wreck reason 12 at 3562 |

  Tuning read out of the bike: backflip time 0..60, torque 800..1400 (floor 1.25 × 800 = 1000), rate
  +500 / −2000, airborne pitch wreck angles 1.3963 down / 1.4835 up, authority ramp 1.0 s.
- **Wrote `the_humans_backflips`** in `crates/game-sim/tests/air.rs`. It keeps the index above as
  consts and checks every number back against the capture. Then, for each tick with no
  wreck in progress, it gives `air::backflip` the original's flip state at the end of the tick before
  and that tick's inputs, runs two substeps of dt/2, and compares the result with the original's end
  of tick.
- **`air.rs` did not move.** The original never disagreed with it. The four ticks it does not
  reproduce are all the stick's timing (below), not the gesture.
- **Added the lip length to P16's journal.**

## What the original does

The backflip step (`./kit fn 0x00256b50`) runs once per substep, two substeps per 30 Hz tick
(`./kit fn 0x00257a00`). Its gesture block reads exactly the way `air.rs`'s `rotate` does. On the lip,
stick y below −0.8 arms; armed and stick above +0.8 fires. The torque is
`map_clamp(|R+0x630|, min_time, max_time, 1.25·min_torque, max_torque)`, and grace is 0.6. Off the lip,
both flags clear.

`R+0x630`, which `air.rs` calls `stick_hold`, is not a time. In this run it reads 0 while the stick is
still and 469.4, −28.57, 32.33 or 54.05 the tick the stick swings. It behaves like a signed speed of the
flick. The 0..60 range turns it into torque, so a fast flick gets the full 1400. P16 asked whether
`MaxBackflipTime = 60` is seconds; this answers it, and not as a time. Where the value is written
(`./kit fn 0x00250fb0` copies it from the controller's `+0x31c`) is not traced further.

## How it was proved

- **A PINE recording compared tick by tick:** `context/capture/acceptance/recording2-backflips.csv`,
  1894 rows. A second capture with `KIT_FRAME_DELAY=0.020` (`context/capture/s31/late20.csv`) is
  identical on every flip and stick column, so the rows are settled end-of-tick snapshots, not torn
  reads.
- **`the_humans_backflips`.** Fed the original's state, the port reproduces the original's flip state
  on every tick outside a wreck but four: armed, fired, grace, torque to the bit, turning and applied.
  - It arms on the original's tick for all three flips.
  - It fires on the original's tick for flips 1 and 3, and chooses each torque to the bit.
  - On every turning tick of all three flights but two, it turns the bike with the original's torque
    to the bit, and it ends flips 1 and 2 on the original's tick.
  - The test pins the four exceptions by tick, so a fifth one fails the test.
  - The measured rotation (turns, peak rate) is asserted against the capture's own chassis rates. The
    port's `flip_torque` is not integrated into a rotation here (see below).
- `./kit check`: TESTS PASSED. `./kit audit`: ok.

## Not verified / not 1:1

- **Which stick value each substep sees is unresolved, and four ticks hang on it.** The test feeds
  both substeps of tick T with row T−1's stick, and that explains every arm and fire but two.
  - At 3291 the port fires a tick early on row 3290's +0.843; the original fires at 3292.
  - At 3518 the port arms a tick early on −0.832; the original arms at 3519.
  - At 2553 and 2572 the original's torque moved by what one substep at a stick of about −0.37 (and
    −0.04) gives, while the row before says −0.43 (and −0.15).

  In all four the stick was moving through mid-travel within the tick, so the substeps saw values that
  no end-of-tick row holds. I tried and rejected these models:
  - row T's stick (0/7 events);
  - row T−2's stick (2/7);
  - the raw pad halves A or B at T−1 or T−2 (at most 5/7);
  - the torque's hold value. It is the same on the rows either side of every fire, so it cannot tell
    the ticks apart.

  The stick is not a pure function of either pad half's bytes, so a filter or state sits between
  them. A per-video-field capture (`vsync_addr` = the pad stamp) dropped most fields (9.8 rows/s) and
  gave one row per tick, which settled nothing. To settle it, record the stick at substep granularity,
  for example with a PINE breakpoint or a write-watch on `R+0x62c` around frame counters 3289–3292,
  and see when it is written relative to the two backflip calls.
- **The rotation is the original's, not the port's.** The turns and pitch rates are summed from the
  capture's chassis body angular velocity. The test does not integrate the port's torque through a
  chassis, because `air.rs` only adds the torque to the moment and the rigid body is not part of this
  step. What it does prove is that the port's torque is the original's on each turning tick.
- **Inputs are the end of the tick before, for both substeps.** Chassis `vel.y`, `on_ground`, contact
  speed, brake and touching all change between substep 1 and substep 2 in the original. That matters
  only where a condition flips within a tick. No mismatch traced to it, but substep 2's inputs are not
  checked.
- **Ticks during a wreck are left out.** From 4124 (flip 3's wreck) the original keeps `flipping` up
  through the crash, while the port, fed those rows, ends it. I take this to mean the wreck path stops
  calling the backflip step, but I did not read the caller to confirm it.
- **The hold's units are inferred, not traced.** "A signed flick speed" is from its values alone.
- **The bike actor columns differ from C6's capture in 7–8 rows in 2374–2408**, and `rng_state` differs
  in 96 rows. Neither touches anything measured here, and every rider and flip column is identical, but
  the two playbacks are not byte-identical.
- **The lip length is end-of-tick rows**, the same sampling P16 used, not per substep.

## Next

`./kit done` lands this. For a follow-up, settle the substep stick timing as described in the first
bullet above; once it is known, `HUMANS_MISSED` should shrink to nothing.
