# 1 — fixed sim rate (FINAL)

Task P52. 2026-10-21.

## What I did

- **The problem.** The shell stepped the game once per drawn frame and passed in a host-clock Timer0.
  The engine's `DeltaTime` averages 30 frames and caps dt at 0.03333, which gave two failures:
  - At 60 fps drawn, the average was about 1/60, below the cap. The offline ride ran at dt ≈ 1/60
    rather than the original's 30 Hz ride. Every capture row has dt = 0.03333.
  - At 5 fps drawn, every frame was capped. The world ran at a sixth of real time (15 ticks in 3 s).
- **`crates/openmx/src/pace.rs` (new)** keeps the schedule:
  - `ps2_clock(frame)`: the Timer0 a steady NTSC PS2 reads at frame *n*. That is 576,000 Hz, wrapping
    at 57,600, with one 2-field frame of 1001/30,000 s (19,219.2 counts).
  - `tick_clock(tick)` puts the ride after a load: its first tick is a long frame (dt 0.033, ring
    refilled). `after_load` primes the engine so this holds from the very first tick.
  - `Pacer` owes one tick per `pace::TICK` (1001/30,000 s) of wall time since the ride started.
    `--frames` caps the tick count and `--seconds` caps its due time. A gap of more than `CATCH_UP`
    (0.5 s) is dropped, so a stall runs one tick and the ride goes on, with no spiral.
  - `blend()` gives how far between the last two ticks the picture is.
  - `Blend`: matrices lerp, then their axes are rescaled to the lerped length, so a turn does not
    shrink the model. A jump of more than `SNAP` (30 ft), such as a put-back or a teleport, is drawn
    where it lands. The camera blends eye/at/fov, and snaps on a cut.
- **`main.rs` ride loop:**
  - `loop { if pacer.owed(now) { …one tick…; continue } …one picture… }`. The tick half is the old
    frame body, unchanged, wrapped. It polls the window, samples the pad or the tape, steps the menu or
    the sim, then animation, camera and sound.
  - Input is sampled once per tick, and a tape or recording is one line per tick. A drawn frame
    behind by several ticks runs them all, each with its own pad sample, before it draws.
  - The picture blends seat 0..n's bike placement, chassis pose and rider palette (ragdoll or posed),
    the camera, and the biplane between the snapshot before the last tick and now. The world is static
    apart from those. The blend is render-side only: `Shown` is built from the seats and never written
    back.
  - `--draw-fps N` forces a picture rate. Headless defaults to 60 (as before), a window to uncapped.
  - Online (N3) keeps its own 60 Hz pacer with `rides::steady_dt()`, so the lockstep is unchanged bit
    for bit (follow-up N6).
  - "ride hash" is now printed for headless rides too. The last line reports the ticks and where
    seat 0 ended.
- **`audio.rs`:** `Sound::frame` is stepped once per tick with that tick's dt. It used to take a
  fixed 1/60 per drawn frame, which would run P49's engine note at half speed against 30 Hz ticks.
- **From opentennis:** looked first. `hst` is a Bevy app with per-system 60 Hz tick counters
  (`crates/hst/src/main.rs:66`, `Clouds::ticks`), not one fixed-step loop with interpolation, and its
  game reads no Timer0. So nothing was taken. The idea is the same, but the code is ours.
- **S55.** Its note is added to its journal (see below, and its "After P52").

## What the original does

- `DeltaTime::update` (game-sim `tick`, already ported bit for bit): Timer0 ms since the last frame,
  pushed onto a 30-entry ring. dt is the ring's mean, capped at 0.03333. A frame over 0.1 s gives
  0.033 and refills the ring.
- **The clock it reads.** The dt ring was read at its oracle address (`research/oracle.txt`, dt_ring)
  in five save states and rotated to start at the write index. Each holds only 33 and 34 ms, summing to
  1001 ms over 30 frames, which is the NTSC frame of 1001/30,000 s. All five patterns are windows of
  `ps2_clock` at some phase. An exact 1/30 s clock (sum 1000) matches none of them. So the ring mean is
  1001/30,000 > 0.03333, and the original rides every frame on the cap.

## How it was proved

- `pace::tests::the_ps2_clock_is_the_originals`: the five rings from the save states are each found
  in `ps2_clock`'s deltas over 300 frames. The falsifier is a 19,200-count clock, which matches none.
- `pace::tests::a_ride_starts_after_a_load_and_settles_on_the_cap`, `the_schedule_owes_the_wall_clocks_ticks`
  (7 ms vs 200 ms sampling gives the same 90 ticks; `--seconds 1` gives 30; a 10 s stall runs 1 tick),
  `a_blend_turns_without_shrinking_and_does_not_slide_a_jump`.
- `crates/openmx/tests/fixed_rate.rs`, the binary ridden headless on free06 for 3 s:
  - `a_ride_drawn_at_5_fps_is_the_ride_drawn_at_60`: a full-throttle tape at 5 and 60 fps. Both take
    3.0–3.6 s of wall time and run 90 ticks, with the same ride hash and the same end position
    `[2893.5, 296.44, 2863.55]`. That is more than 20 ft from an untouched ride.
  - `the_engines_dt_is_the_same_every_tick_at_any_drawn_rate`: `--record` at 5 and 60 fps writes
    identical dt bits for all 90 ticks. They start at 0.033, are non-decreasing, reach 0.03333 before
    tick 30 and hold it.
- A window on :0 drew about 220 fps and ran 180 ticks in 6 s.
- `./kit check`: TESTS PASSED. Replays, tapes, acceptance and the netplay tests are all inside it.

## Not verified / not 1:1

- **The load ramp is the decompiled rule, not something seen.** The first tick is a long frame (0.033)
  and dt climbs to the cap over about 27 ticks. Every capture starts already capped, so no capture shows
  a ride's first second.
- **`LOAD_FRAMES` (19 s of frames before the ride) is a stand-in.** Only its "more than 0.1 s"
  matters to dt. The Timer0 phase it gives is not the original's load length.
- **`CATCH_UP` (0.5 s) is our choice.** The original has no catch-up: a long PS2 frame (over 0.1 s) is
  simply dt 0.033. Our dropped time is lost the same way, but the threshold is ours.
- **Online is still 60 Hz with dt ≈ 1/60**, not the original's 30 Hz. Moving it would change the N3
  lockstep and its tick length. Filed as N6.
- **The software (no-GPU) path blends only the root placements** (bike and plane), not the chassis or
  rider poses. That path is a fallback.
- **The interpolation is ours and was not matched to anything.** The picture lags the simulation by up
  to one tick (33 ms). The blended palettes are lerped and length-rescaled, not orthonormalised. Parts
  on the GPU path ride the blended chassis. It was watched only briefly in a window, not side by side
  with the original.
- **The menus, pause and window events now run at the tick rate**, 30 a second, not per drawn
  frame. The odometer is still stepped per picture, from the last tick's position. P53's taps are queued, so no press is lost, but a menu cursor moves at most 30 times a
  second.
- **Sound is stepped per tick**, so at 5 fps drawn, six ticks' sound frames are queued in a burst
  before each picture. Not listened to at low frame rates.
- **S55's remaining checks** (the rider body's blend and the camera at touchdown) were not done here.

## Next

- N6 (online at 30 Hz). If a landing snap is still seen, do S55's rider-body and camera checks.
