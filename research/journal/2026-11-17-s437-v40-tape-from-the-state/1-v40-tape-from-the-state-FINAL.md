# 1 — v40 tape from the state (FINAL)

Task S437. 2026-11-17. Base efbfe61eb.

## What I did

- **Checked what the shell rides.** `--track free09 --play` rides the shell's own default race125p,
  placed fresh at free09's start line (4310, 208, 3316), standing still. The shell has no way to start
  from a save state. first_recording's state holds a 125 (race125p, cr125_faisst) already rolling
  (0.6 at 4309.6, 208.0, 3319.6). Its clutch timer is 1.53, so the launch ramp is long spent. A fresh
  rider's timer is 0, as the original's constructor leaves it (P154). That runs `mv::clutch`'s launch
  ramp: 3× torque at a standstill, fading over 1.5 s. V120's probe.log shows the port gaining ~2 a tick
  off the line where the original gains ~1, with the rear spinning all the way.
- **Captured the original tick by tick.** `research/s437/capture.sh` is C6's play_capture.sh with
  S58's `watch/s58-slip.toml`. It makes `research/v40/hold.pad` a from-state .p2m2 (`research/c15/
  tape.py`), plays it from first_recording's state on copy 1 over AT-SPI, and records every tick from
  frame 2774. That gave 292 rows (2774..3065) with no gaps, at NominalScalar 0.05. The files are
  `context/capture/s437/hold.csv` and `context/recordings/s437/hold.p2m2` (not committed).
- **Replayed the port from the same state.** Added `the_v40_tape_replays_from_first_recordings_state`
  to `crates/game-sim/tests/replay.rs`. It goes through `replay_a_save_state_case`, as S106's braked
  slopes do: the state's chassis, wheels, balance, engine, air and levers are seeded, and the pad is the
  one the original read.
- Nothing from `../opentennis`: this is a replay of the port's own sim.

## What the original does

On the capture (tick = row), from first_recording's state:

- Rear spinning on ticks 5–7 only off the line. Then again from the lip (43) through the flight to 96.
- Front locked a tick at the landing (102), rear locked under the brake, 113–123.
- The brake takes `ch_speed` to 7.0 (tick 141). Once the brake is off (136), the bike rolls down the
  slope's foot, and the rear spins on 146–147. Speed tops out at 13.7 by 181.
- The held brake from 196 stops it at 4329.5, 138.2, 3487.9 (speed 0.02).

This is V120's hold.log ride, recorded as a capture.

## How it was proved

- **A save-state capture, replayed bit-exact.** `the_v40_tape_replays_from_first_recordings_state`:
  293 ticks, **0 of 112 lanes part**. The lanes cover the chassis' path, velocity, quaternion and centre
  of mass, both wheels' spin, local velocity and force, the spinning/locked/sliding flags, the surface,
  the levers and the engine. The test asserts the path and the slip flags agree through tick 292, the
  last row. It skips with a line when the capture or the state is missing.
- So `tire.rs` and `ride.rs` ride this tape as the original does. V120's differences come from the
  shell's start, a fresh bike at the start line, not from the sim.

## Not verified / not 1:1

- **The shell's fresh start was not compared with a fresh start in the original.** The original was
  only ridden from first_recording's state, where the clutch ramp is spent. The claim that the shell's
  long launch spin is the unspent ramp rests on P154's finding (the original's constructor leaves the
  timer at 0, and a settled fresh bike matches P21's capture lane for lane). It also rests on the ~2 vs
  ~1 a tick gain in V120's logs. No original ride from a fresh freeride start was captured on this tape.
- **The shell's ride was not re-run.** The difference is explained by where it starts, and no product
  code changed, so V120's probe.log still stands for it.
- **The marks themselves were not compared.** Tire tracks follow the flags, and the spin/lock flags
  agree on every tick, but the strips' vertices were not read from the original.

## Next

Done.
