# 1 — mxvatv 360 cross from the original (FINAL)

Task MD248. 2026-11-18.

## What I did

- `crates/game-sim/src/vehicle_anim.rs`: the two 360 states left the played-once arm. They have their own
  arm now, the way R43 read the original:
  - One clip (mode 1), faded in from the saved pose by the ramp. The restart already saves the pose.
  - Rate in: the rider's 2 a second, the bike's 3. Not the body's gesture rate (4) any more.
  - Rate out of a 360: the bike's 3 for any next state. The rider's 2, except the wreck and start-line
    groups, which keep their own 0.25 (the original's wreck poses and pause are 0.25 too).
  - The fade falls by `adda(0, fade) msub(rate, dt)`, MX vs. ATV's own form. The bike's fade holds at 1 on
    the tick the 360 starts or ends, and falls from the next tick. The rider's falls on the same tick.
  - Phase: the stunt clock (dt on the first tick, dt more each tick). Rider: `min(clock / 1.5, 1)`.
    Bike: `clock / clip length`. The clock is kept in the reverse pair's clock field, which nothing reads
    while a 360 runs. That keeps the driver's wire and hash layout as it was.
- `Library::bike` (new flag) says the table is a bike's. `crates/game-render/src/vehicle.rs` sets it on the
  chassis, beside B10's `restart_tricks`.
- MD78's 0.25 s cross (a smoothstep `Player::play` transition) is gone. The question in the brief is
  answered: the old 360 cross was **smoothstep**, not linear. The ramp path draws the saved pose with a
  plain slerp/lerp by the ramp (`skin.rs` `lay`), so it is linear now.
- `crates/game-sim/tests/mxvatv_stunt_cross.rs`: three new tests (below).
- `crates/game-render/tests/whip_tricks.rs`: `a_360_throws_the_rider_off_the_pegs` keeps its thresholds:
  pelvis out 1.12 ft and down 1.33 ft, feet 2.35 ft off the pegs, settled 0.000 after. The V69 test
  `a_360_keeps_the_hands_on_the_bars` had a floor on how far the *raw clips* (not the drawn rider) part
  the hands on the exit fade: 0.82 ft under the old smooth step, 0.44 now. Floor moved 0.5 -> 0.4 and its
  window 15 -> 30 frames (the exit fade is now half a second). The drawn hands still sit on the grips (0.000).
- Filed R47 (low): the original's stunt clock ends the stunt at the row's 1.5 s (see below).
- Nothing from ../opentennis: it has no stunt animation.
- Ran the pack tests with `mods/tricks` linked to the main checkout's (git-ignored), or they skip.

## What the original does

See R43's journal (2026-11-17-r43-mxvatv-360-real-cross). New here:

- The stunt clock step (`./kit dis 0x004e4900 0x004e49b0 -p atv`, bike vt `+0x4e8`): clock `+= dt`, then
  phase = `min(clock / Duration, 1)`. If the phase is below 1 it is stored and the call answers 1. If it
  reaches 1, the clock is **zeroed** and the call answers 0 — the stored phase stays at the last value
  below 1. So the rider's clip never plays at exactly 1 under it.

## How it was proved

Function oracle on state04's RAM image, SLUS-21104's own code beside the port, every tick
(`crates/game-sim/tests/mxvatv_stunt_cross.rs`):

- `the_ports_rider_crosses_a_360_as_the_original`: the port's rider driver against the original's rider
  layer, 46 ticks into a 360 and 20 out. Every tick's ramp equals the original's fade bit for bit. The
  360 clip's phase equals the original stunt clock's stored phase bit for bit for ticks 1..45. On tick 46
  the original's clock zeroes (stunt over); the port holds phase 1.
- `the_ports_bike_crosses_a_360_as_the_original`: the port's bike driver against the original's bike
  animate, 20 ticks in, 20 out. Ramp bit for bit, held at 1 on both change ticks, 0 after 11 ticks.
- `only_the_360_takes_the_originals_fade`: without a 360 the rates are unchanged (pack extra 4, airborne
  3.25, wreck pose out of a 360 0.25).
- `./kit check -- -p game-sim --test mxvatv_stunt_cross`: 6 passed. game-render whip_tricks, pack_clips,
  trick_chain (14), anim, body, vehicle (34), game-sim vehicle_anim, rider_anim, motion, ride, foot_down,
  biplane_anim (109) pass.

## Not verified / not 1:1

- **The 360's end.** The original's stunt clock ends the stunt when its phase reaches 1 (1.5 s). The port
  keeps the 360 state until the spin clip state clears (round or landed), so a 360 longer than 1.5 s holds
  the clip's last frame in the port. What the original's caller does on that end was not read. Filed R47.
- **Which tick the stunt clock starts on.** The port's clock is dt on the 360's first tick. That matches
  the original's clock started from 0 on the same tick as the rider's state change, which is how the test
  runs it. The real order of the stunt clock step, the rider layer and the bike animate inside one game
  frame was not seen live.
- **The rider's rate out into a turn.** The original's turn state takes 3 a second (1/3 s). The port's
  rider has no separate turn state (Unleashed's pose grid covers riding and turning), so out of a 360
  into the grid takes 2 (the still rate). Only a 360 that ends turning on the ground would differ, and a
  landed 360 with its clip still on is a wreck.
- **Not watched live.** The fades and phases were run in the original's code on a save state, not seen
  in PCSX2 during a real 360. Same limit as R43.
- **The bike's phase past its clip.** The bike plays at clock / clip length, clamped to 1 by the blend.
  What the original's bike clip does past its end was not run (the stunt ends at 1.5 s, after the 1.467 s
  clip, so 1 tick of difference at most).
- **Only state04's numbers and only the right 360**, as R43.
- **The bike's handling of other tricks is unchanged.** Only the 360 got the bike's own fade here; other
  pack states still take the body machine's paths (B10 covers gesture slots).
