# 1 — p30 a backflip on the real bike (FINAL)

Task P30. 2026-10-20.

## What I did

- Read the per-tick input chain from the stick to the air steps (below), and found the port's air
  read the wrong stick and the wrong "hold".
- Confirmed it on S31's recording before touching the port: squaring the stick in
  `the_humans_backflips` (sign kept) takes its four listed misses (2553, 2572, 3291, 3518) to none.
- Ported it. `ride::Held` now carries what the pad read and the limiter leave on the bike: the
  squared stick (+0x628/+0x62c), the d-pad flags, the rate-limited lean pair (+0x670/+0x674, at the
  `DPad` rates on a digital axis) and the flick speed (+0x630) from a 32-sample `ride::Flick` ring.
  The air steps read the squared stick and the flick; the ground steps, the rider's pose and the
  camera read the limited lean, which is what they read before under the name `held.stick`.
  `STICK_HELD` is gone. The trick gesture still reads the player-range stick (`rider.rs`).
- The shell's arrow keys are the d-pad now, not a ramped left stick (`pad.rs`'s `keyboard`). The
  original's d-pad is its digital stick: it forces ±1 into the stick, flags the axis, and the bike
  then uses the `XDPad`/`YDPad` lean rates and the `DPad` preload times. On the key ramp the stick
  crossed at about 7 a second, a twentieth of a thumb's flick, and could not make three crosses on the
  125's 27-frame lip at all (the keyboard ride's preload never fired). `IJKL` stays on the ramp.
- P16's flip came off the stand-in minute in both crates (the minute is ten legs and 3600 frames
  again; `RIDE_FLIP` is gone). Its numbers were written for the old reading of the stick and stopped
  meaning anything. The flip is its own ride on the disc's `race125p.phy`: `THE_FLIP` in
  `crates/game-sim/tests/ride.rs`, and `crates/openmx/tests/ride-flip.pad`, frame for frame.
- `--trace-stick-y F:Y,...` for the shell's `--trace-feel`: a schedule for the stick's y, plus the
  gesture's columns (stick, lip, preload, flip, nose).

## What the original does

Per tick, for the human rider (record P = bike + 0x20):

1. **Pad listener** (`./kit fn 0x0025d240`, registered by `0x0025d810` on the app's listener list,
   dispatched by `0x001321d0`). The engine tick calls the dispatcher after the pad update (phase 1) and
   after the sim (phase 2), and the DMA waits (`0x00177e30`, `0x00191040`) call it again whenever a wait
   passes 4 ms. Each call pushes one sample into a 32-slot ring on P: head P+0xa0, time P+0xa4+4·slot
   (Timer0 ms × 0.001 — the wall clock), and four axis values P+0x124+0x10·slot (+0 LX with the d-pad's
   left/right as ∓1, +4 LY with up/down as ∓1, +8/+0xc the right stick). Save states show ~5 samples a
   frame, a count that depends on how long the GS waits — emergent, not a constant.
2. **Player per-tick** (`./kit fn 0x0025e910`): copies the four axes to P+0x324..0x330, then for each
   axis computes a **flick speed** into P+0x638+0x18·i by scanning the ring oldest to newest: a sample
   past ±0.8 is remembered (time, value); the next one past 0.8 on the *other* side, if the remembered
   time is > 0, gives speed = Δvalue / Δtime, kept if larger in magnitude than the best so far; the
   memory is cleared either way. Then the actors' vt+0x20 get the sticks and P+0x334's command table.
3. **pad → gesture** (`0x0027ff90` → `0x00250fb0`): bike +0x628/+0x62c = raw player-range stick (d-pad
   overriding to ∓1, as `trick::stick`), +0x630 = P+0x650 = **LY's flick speed**, +0x620/+0x624 = the
   d-pad flags, +0x640 the brake lever, +0x9fc the trick-button group, +0xa00 the eight-way direction.
4. **Input rate limiter** (`./kit fn 0x00251a50`, the bike's vt+0xec before the ground step): for the
   human (P+0x34 == 1) +0x628/+0x62c become **x·|x|, y·|y|** in place; then the lean pair +0x670/+0x674
   walk toward them at `XDPadRateLimit`/`YDPadRateLimit` (+0x668/+0x66c) when that axis's d-pad flag is
   set, else `X/YAxisRateLimit` (+0x660/+0x664), as `now + min(max(want − now, −r·dt), r·dt)`;
   +0x670 is zeroed while +0x610 is clear.
5. **Air steps**: preload (`0x00256940`) and backflip (`0x00256b50`) read +0x62c (squared raw y),
   +0x630 (the flick), +0x640 and +0x624/+0x9fc; airborne control (`0x00256e50`) reads +0x628/+0x62c
   and the rate-limited +0x670.

So the answer to the brief's question: **the gesture reads the squared raw stick, with the d-pad
forcing ±1; not dead-zoned beyond the player range, not rate-limited**. The rate limiter's XDPad/YDPad
rates feed only the lean pair, which the air reads through airborne control's lean. The "held" input
of the backflip is not a time the stick has been held but the speed of the last full-travel flick.

`air::Step::dpad_dir` is fed +0x9fc, the trick group, not a d-pad direction.

Addresses and offsets are in `research/oracle.txt` under "P30".

## How it was proved

- `crates/game-sim/tests/air.rs`'s `the_humans_backflips`: with the capture's stick squared, every
  tick of recording2's three backflips reproduces (was four listed misses); `HUMANS_MISSED` is empty.
- `the_flick_is_the_originals_clock_aside` (same file): `ride::Flick` fed the capture's own stick and
  frame times, against the original's +0x630 on every row. 12 flicks. The edges fall only where a
  flick ends, the signs agree, the two-tick flicks are within 1%, and the one-tick flicks are 0.1× to
  1.01× of the original's (469.4 captured, 56.3 from the port's per-tick ring, for example), which is
  the clock difference below. The backflip torque on the 125 is clamped at `MaxBackflipTime` 60 either
  way, so both give the full torque for any real thumb.
- `it_flips_off_the_lip` in `crates/game-sim/tests/ride.rs`, on `race125p.phy` off the disc: the
  preload (about tick 159 of 60 Hz) and the backflip (about 168) both fire on the lip, the lip holds 27 ticks, the
  bike goes round once and lands upright. The forward stab that ends the turn lands it anywhere from
  250 to 290; without it the 125 comes down on its back.
- `it_flips_off_the_lip` in `crates/openmx/tests/ride.rs`: the same ride through the shell's pad
  reports, then again on the arrow keys through `pad::Pads::build`. The keys fire the backflip within
  two frames of the stick. Both crates' tests were run once with the disc skip turned into a panic, to
  show they ran rather than skipped (they pass silently without a disc).
- The shell, on free06 with race125p (the start line, `--trace 420 --trace-feel --trace-gas
  --trace-stick-y 0:0,229:1,236:-1,239:1,242:0,295:-1,305:0`, 30 Hz). The first big jump after the
  start has a lip of 17 frames (232–248), the same as the human's three flips in the original (17–19).
  Preload fires at 236, the backflip at 239, the bike leaves at 249 and lands at 349 (3.3 s), with no
  wreck. Every sixth frame (speed, vel_y, up, ground, stick, lip, preload, flip):

  ```
  234 95.4  8.3  0.989 1  1  1 0 0
  240 65.2 60.5  0.824 1  1  1 1 1
  252 55.0 60.7  0.621 0  0  0 0 1
  264 53.4 46.2 -0.526 0  0  0 0 1
  276 52.0 32.3 -0.974 0  0  0 0 1
  288 50.8 18.8 -0.098 0  0  0 0 1
  300 49.7  5.7  0.876 0 -1  0 0 1
  312 48.7 -7.2  0.976 0  0  0 0 0
  354 60.5 -13.8 0.949 1  0  0 0 0
  372 74.6 -22.3 0.950 1  0  0 0 0
  ```

  The stab anywhere from 290 to 305 (ten frames) lands it, up 0.78–0.94 on touchdown; a five-frame
  stab does nothing and the bike comes down upside down. The shell window itself was not driven: no
  keystrokes reach it on this desktop, so this is the shell's own frame, headless.
- `./kit check`: passes. Hash pins moved (the ride's state grew): `HUNDRED_TICKS`, the ride's leaf
  count (543) and size (2088), `Sim`'s size (7408) and leaf count (821), and the platforms vectors.

## Not verified / not 1:1

- **The flick ring's clock.** The original stamps samples with the wall clock and takes ~5 a frame,
  however many DMA waits pass 4 ms; that cannot be reproduced and would not be deterministic. The port
  pushes `dt × 150` copies of the tick's stick at the simulation's clock (`ride::Flick`), so a flick
  inside one tick is the stick's travel over a fifth of a tick's samples, and a slower flick is the
  same speed as the original's. A one-tick flick can be up to ~8× faster in the original; the 125's
  torque saturates long before, so a backflip does not see it. A bike whose `Max*Time` range is in
  the hundreds would.
- **+0x610** (the lean pair's enable) is assumed set while riding. Not traced.
- The limiter's gas, brake and clutch lanes were not compared beyond what `Held::step` already did;
  the lever walks are unchanged.
- The trick gesture (`rider.rs`) still reads the unsquared player-range stick. Not re-checked against
  the original's `+0xa00` reader this session.
- **The keyboard is the port's own.** The original has no keyboard. Mapping the arrows to the d-pad is
  the closest thing it has, and it changes how the shell steers on keys: the lean walks at
  `XDPadRateLimit` (2.75 on the 125) where the key ramp used to feed `XAxisRateLimit` (4), and the
  ground's centre-of-gravity shift and the air control get ±1 at once. Not ridden by a human.
- The flip ride runs at 60 Hz in the tests and 30 Hz in the shell's trace; the original runs 30.
- The minute lost its flip leg. It is ridden on the disc's bike instead, so without the disc there is
  no flip test at all.

## Next

`./kit done P30`. A human should try the backflip on the keyboard in the shell: arrows down as the lip
arrives, up, down, then up briefly in the air to stop the turn.
