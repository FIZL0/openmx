# 1 — the minute on the discs bike (FINAL)

Task P26. 2026-10-20.

## What I did

- **Added section (11) to `crates/game-sim/tests/ride.rs`**, called `the_minute_on_the_discs_bike`.
  - **The bike.** It reads `MX.PSR` the way `tests/replay.rs`'s `Disc` does. The code is copied, not
    shared, so that the test file reads on its own. It takes `race125p.phy` out of the first bike archive
    that has it (`Bike\125sx.res` on this disc) and hangs the wheels on the `125_faisst` model's
    `TireFront`/`TireRear` nodes. That is how the acceptance fixture builds the human's bike.
  - **The ride.** A whole `Rider`, wrecks on, dropped from 2 ft onto `Flat` at `DT = 0.03333`, which is
    the captures' own `dt` column.
  - **No disc, no test.** Without the disc the test prints a line and returns, the way replay.rs does.
- **The legs**, in order:
  1. 3 s settle.
  2. 20 s flat out from a standstill.
  3. 3 s coast.
  4. 5 s brake, then 2 s stopped.
  5. Gas until 50 ft/s, then 2 s of full right stick with the gas pinned.
  6. Brake and stop again.
  7. The same to the left.
- **One table, then one failure.** Every band is checked and the whole table is printed before anything
  fails, so a single run gives P21/P23/P24 and the drive task each their own line. A wreck, or the bike
  going past `up < 0.5` on any leg, also fails the test.
- **Compared in the capture's units.** The capture's `bike_vel` is the chassis velocity × 0.8. At the
  write site the literal `0x3f4ccccd` goes through `qmtc2` and `vmulx.xyz` (`./kit dis 0x0024d410`, the
  `vmulx` at `0x0024d42c`). I checked this for this task; P22's session found it first, as a memory note.
  So the port's `chassis.speed` is multiplied by 0.8 (`BIKE_VEL_SCALE`) before it is compared.
  P15's bands are raw `bike_vel`, so they are left as they are.
- **Measured r1's one flat brake** off `acceptance-freeride.csv` with a scratch script (not committed):
  56.89 at tick 1036 → 51.30 at tick 1048, which is 14.0 ft/s² over 0.4 s.
- **Shipped `#[ignore]`**, with a reason that names what fails. `./kit check` passes: TESTS PASSED.

## What the original does

These are P15's numbers, re-stated as the bands the test asserts. Speeds are in capture units; the
original's rides are at 30 Hz.

| band | original | how the band was set | port, P26's first ride |
|---|---|---|---|
| launch 0.5 / 1 / 2 s | 8.8–20.2 / 19.4–32.3 / 34.2–56.9 | min..max of the 4 launches | 14.7 / 29.0 / 53.4 ✓ |
| 0 to 30 / 0 to 60 | 0.93–1.53 / 2.2–3.4 s | min..max (r1's 5.5 s with air left out) | 1.07 / 2.40 ✓ |
| accel 0–20 / 20–40 / 40–60 | 25.3–25.8 / 15.9–22.1 / 16.4–17.6 | the two rides, ±10% | **32.1 / 25.9 / 20.7** |
| top speed, upright | 75.78–79.89 | the two maxima, ±10% (68.2–87.9) | **97.6** |
| coasting decel, median | 3.4–4.6 | ±10% | **9.4** |
| brake, mean over first 0.4 s | 14.0 (r1, flat) – 44 (r2, up-slope) | ±10% outside each | 36.2 ✓ |
| held turn rate, after lean-in | 0.6–2.5 rad/s | P15's range, sign = stick | +2.473 ✓ / **−2.509** |
| speed kept, 1.1 s into a held turn | 0.853 (r2) | floor at −10% (0.768) | **0.670 / 0.647** |

No wrecks on any leg, and the bike never went past up 0.5.

## How it was proved

- **The original's side** is P15's measurement off the two 30 Hz captures of the original's RAM, plus
  r1's flat brake, re-read for this task off `context/capture/acceptance/acceptance-freeride.csv`.
- **The port's side** is the disc's own bike, read by the port's loader, ridden through `Rider::frame`
  by the test. It was run once with `./kit check -- -p game-sim --test ride -- --ignored --nocapture
  the_minute_on_the_discs_bike` and failed on the eight bolded lines above. The full `./kit check`
  passes, with the test ignored.

## Not verified / not 1:1

- **The test is not a match. It is `#[ignore]`d and fails** on eight lines: the three accel bands, top
  speed, coasting decel, the left turn rate (by 0.009) and both turn-speed-kept lines. Fixing them
  belongs to the drive task, P21, P23 and P24.
- **Several bands are guesses about the original's range:**
  - **Most bands have only one or two samples.** The accel, top-speed, coast and brake bands are ±10%
    around them; that margin is my choice, not a measured spread.
  - **The top-speed band is not a terminal speed.** Both maxima are at the foot of a drop with the bike
    still accelerating (P15, P21).
  - **The brake band is wide and mixes two different brakes.** r1's was on the flat with the stick
    pulled back; r2's was on a 0.6 up-slope, with gravity helping. Neither ride holds the brake to a
    stop, so the minute does not assert a stopping time.
  - **The turn-speed-kept floor comes from one turn** (r2, 61.4 → 52.4), and the gas was not
    necessarily full there. The test enters its turns at 50 ft/s with the gas pinned.
- **The flat floor is not Free09.** The original's launches came out of real riding (downhill, off a
  landing, feathering the throttle). The port's are clean standstills on a level plane, which is the
  task's specified floor. The launch passing here does not mean the drive is right: the per-band accel
  lines, which measure the same pull, are 20–25% over.
- **The ×0.8 is read off the binary but not proved by a capture in this task.** I read one `vmulx` by
  0.8 at the write site. P22 is the task that proves it. If it turns out wrong, every speed band in this
  test moves by 1.25.
- **The port numbers here differ from P15's shell trace** (P15 had 33 ft/s at 0.5 s raw, which is 26 in
  capture units, against 14.7 here). The shell rode Free09's lane with the `.phy`'s own mounts or the
  model's, through the shell's frame. This test uses the flat floor, with the bike dropped and settled.
  I did not chase the difference.
- **No carried wreck body.** The rider gets no `carries_wreck` part, the same as replay.rs's disc
  fixture, so a wreck would be detected but not ridden out. None happened.

## Next

When the drive, P21, P23 and P24 land, run the ignored test. Take the `#[ignore]` off once every line
is inside its band.
