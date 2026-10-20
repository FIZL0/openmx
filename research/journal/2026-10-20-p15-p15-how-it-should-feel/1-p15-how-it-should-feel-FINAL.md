# 1 — p15 how it should feel (FINAL)

Task P15. 2026-10-20.

## What I did

- **Captured recording2 off the original** (it had never been played back), so both human rides could be
  measured. The capture is `context/capture/acceptance/recording2-freeride.csv`: 1,894 rows, frames
  2374..4267, no torn reads, no counter gaps, `bike_type` 4 and `bike_ctl_src` 1 on every row. It is only
  `watch/acceptance.toml`'s columns, so it is **not** C6. C6 wants the trick and air columns as well. The
  capture sits in this slot's git-ignored `context/` and is not committed. How it was taken, since
  plan/HUMAN.md's playback item had only done it by hand:
  1. Pause the copy first (System → Pause over AT-SPI). A pause pressed *during* the state load does not
     take.
  2. Tools → Input Recording → Play, with the copy's path set into the file dialog's text field and Open
     pressed. The state loads and the counter lands on the recording's own 2373, still paused.
  3. Start `./kit record` on a copy of the spec with `state` removed and `warmup = 0`, with
     `KIT_FRAME_STALL=300`.
  4. Unpause over AT-SPI.

  The human's `recording2.p2m2` was never opened. A scratch copy was played, and the original's md5 is
  unchanged at 1e5091236f40915ad1e5942bce20287d.
- **Gave the shell's `--trace` what a feel measurement needs.** All additive, and only in
  `crates/openmx/src/main.rs`:
  - `--trace-feel` prints one short line a frame: speed, velocity, position, heading as `atan2(m20, m22)`,
    up, on-ground and wreck.
  - `--trace-stick X` holds the stick over.
  - `--trace-switch N` rides the throttle alone for N frames before the other flags' controls take over.
  - `--trace-at X,Z,DEG` starts the bike on the ground somewhere other than the start line.
  - `--phy NAME` picks the bike archive's member by suffix.
  - The trace now prints a `phy:` line naming the member it rode.
- **Found that the shell does not ride the original's 125.** The default asks the bike archive for
  `125sx.phy`, finds none, and takes the first `.phy`, which is `bike\pro125a.phy`. D7's freeride bike is
  Race125P. Every port number below is `--phy race125p.phy`. The default is left alone (P25).
- **Measured both columns** with one script over both kinds of file. The tables are below.
- **Raised the legs** in `crates/game-sim/tests/ride.rs` and `crates/openmx/tests/ride.rs`, identically,
  as far as the hand-written fixture carries them:

  | leg | was | now | fixture measures | original |
  |---|---|---|---|---|
  | launch, 0.5 s | > 3 | > 8 (`ORIGINAL_LAUNCH.0`) | 10.15 | 8.8 – 20.2 |
  | launch, 2 s | > 20 | > 32 (`ORIGINAL_LAUNCH.1`) | 35.04 | 34.2 – 56.9 |
  | turn rate | > 0.1 | > 0.2 rad/s | 0.244 | 0.6 – 2.5 |
  | air off the ramp | > 30 frames | > 70 | 83 (1.38 s) | 2.2 – 2.5 s off ~52 ft/s lips |
  | slowest after landing | > 0.4 × 75.78 | > 0.65 × 75.78 | 53.0 (off at 69.8) | 4–8 ft/s lost off 52 |
  | brake, 2 s | < half | < a fifth | 4.3 from 62.2 | — |
  | brake to < 1 ft/s | (10 s) | < 3 s | 2.1 s | never recorded |

  The fixture's numbers came from a temporary probe test, run once through `./kit check` and deleted.
  Both files gave identical numbers. Top speed stays ±10% of 75.78, because the fixture's drag patch is
  calibrated on that number.
- **Opened a task per gap:**

  | task | gap |
  |---|---|
  | P20 | drive pulls about 2.5× too hard |
  | P21 | no recognisable top speed |
  | P22 | falls too fast and leaves a lip flat |
  | P23 | brake |
  | P24 | a turn scrubs off speed |
  | P25 | the shell's default `.phy` |
  | P26 | a minute on the disc's own bike |

## What the original does

The table compares the original's two rides against the port. The original's two rides are
`acceptance-freeride.csv` (r1, 1,811 ticks) and `recording2-freeride.csv` (r2, 1,894 ticks), both in
`context/capture/acceptance/`. The port is `openmx --track free09 --phy race125p.phy --trace-feel` at the
30 Hz clamp.

Speeds are in the units of the `bike_vel` columns, which the port calls `chassis.speed`. Distances are
position deltas.

| | original r1 | original r2 | port, race125p in the shell |
|---|---|---|---|
| top speed, upright, with the gas held | 75.78 (tick 177) | 79.89 (tick 588) | **121.8**, settled after ~11 s flat out at x=4620 |
| …where | foot of a 70 ft drop, still gaining 7–8 ft/s² | foot of a 68 ft drop, still gaining 8–10 | flat lane, z 2320 → 3470 |
| accel, flat, straight, full gas: 0–20 / 20–40 / 40–60 / 60–90 ft/s | 25.8 / 22.1 / 17.6 / 7.2 | 25.3 / 15.9 / 16.4 / 9.6 | **76 / 43 / 40 / 18** |
| standing launch: to 30 / to 60 | 1.5 / 5.5 s (air in it); 0.93 / 2.2 s (downhill) | 1.53 / 3.37 s; 1.17 / 2.73 s | **0.47 / 1.33 s**, flat |
| speed 0.5 s / 1 s / 2 s into a launch | 9.3/19.4/34.2; 20.2/32.3/56.9 | 8.8/20.3/38.2; 16.3/29.6/49.3 | 33 at 0.5 s |
| held stick (\|x\| ≥ 0.9), rate after the lean-in | +0.94 at 10, −1.55 at 31, −2.20 at 50, −1.97 at 28 rad/s | −1.23 at 19, −2.54 at 35, −1.32 at 54, −0.64 at 57 | 1.62–1.89 at 17, 1.97–2.17 at 33, 2.38–2.46 at 51, 2.46–2.53 at 76 |
| speed kept through a held turn | 61.4 → 52.4 over 1.1 s (r2) | | **76 → 35 over 1.33 s**, gas pinned |
| off a lip at ~52 ft/s: air time | 2.43 s (r1 395, 52.6, vy +24.0) | 2.23 s (r2 273, 51.9, vy +23.4); 2.47 s (r2 1395, 51.6, vy +24.3) | 2.50 s (r1's lip, 52.8, **vy +18.6**); 1.90 s (r2 273's lip, 56.8, vy +19.1); 2.70 s (r2 1395's lip, 54.9, vy +31.7) |
| …distance | 148 ft | 134; 147 ft | 122; 100; 136 ft |
| …peak over the lip | ~13 ft | 12.3; 13.3 ft | **5.0**; 5.2; 14.5 ft |
| speed a landing costs (lip → touchdown → +0.5 s) | 52.6 → 45.0 → 43.3 | 51.9 → 44.6 → 29.7; 51.6 → 44.1 → 42.5 | 52.8 → 45.8 → 64.6; 56.8 → 51.0 → 28.4; 54.9 → 47.1 → 65.0 |
| horizontal speed lost in the air | 2–3 ft/s² | same | about the same |
| vertical velocity change in the air | −25.4 /s² | −25.4 /s² | **−38 /s²** |
| Δposition / (velocity · dt) | 1.254 | 1.25 | **1.00** |
| brake on the flat, square held, gas off | ~25 ft/s² for 0.13 s, then 8 as it pitches up (ticks 1038–1060, stick pulled back) | one brake, on a 0.6 up-slope into a flight: 66.6 → 48.9 in 0.4 s | **54 ft/s² steady** |
| full brake from top speed to a stop | **never recorded** | **never recorded** | 2.47 s from 121.2 (138 ft); 1.2 s from 49 |
| flat coasting decel | median 3.4 ft/s² | 4.6 | — |

What the gaps say, plainly:

- **Drive.** The port's drive pulls 2–3× the original's at every speed band (P20). It tops out 50% over
  anything the original was seen doing (P21).
- **Air.** In the air the port falls about 1.5× faster per velocity unit. Off a lip it leaves with less
  vertical speed at the same horizontal speed, so it flies lower but about as long (P22).
- **Turns.** The turn rates overlap, but the port bleeds speed through a held turn that the original
  keeps (P24).
- **Brake.** The brake is harder than the one flat brake on record, and how long the original takes to
  stop is not on record at all (P23).
- **Landings.** These are the closest thing to a match: both lose about 6–8 ft/s off a ~52 ft/s lip.

## How it was proved

- **The original's side** is measured off two captures of the original's own RAM at 30 Hz. r1 is S8's;
  r2 is this task's, 1,894 rows with no gaps.
- **The port's side** is measured off the shell's own frame on the disc's own `race125p.phy`, over Free09's
  own ground.
- **One script for both.** Both sides go through the same scratch script (feel.py + brakes.py, not
  committed). It reads either a capture csv or a `--trace-feel` trace, with one exception: the port's
  flights come from its own `on_ground` flag, because the capture's ballistic test (Δvy/dt within
  −27.5..−23) does not fit a bike that falls at −38.
- **The flat lanes** were found by probing Free09's ground on a 50 ft grid with one-frame traces.
  x = 4600..4660 sits at y 135 from z 2300 to 3750.
- **The fixture's numbers** come from a probe run through `./kit check`, and the raised legs pass in
  `./kit check`.

## Not verified / not 1:1

- **The original's top speed is not known.** Both maxima are at the foot of a long drop with the bike still
  accelerating, so 75.78 and 79.89 are lower bounds plus gravity's help. No stretch of either ride holds the
  throttle on the flat long enough to reach a terminal speed. P21 asks for that capture.
- **The original's full brake is not known.** Neither ride holds the brake to a stop. The only flat brake
  is 0.77 s long with the stick pulled back and the bike pitching up, so its 25 ft/s² is only the first
  0.13 s's bite, rear and front together, and it is not a clean comparison with a stick-centred brake.
- **Which velocity reading is right is R6's open question.** The original's positions advance 1.25×
  velocity·dt; the port's advance 1.00×. Every speed here is compared in velocity units and every distance
  in position units, so a distance comparison carries that 1.25 (148 vs 122 ft off the same lip at the same
  speed is 1.21).
- **Launches are not the same launch.** The original's four come out of the riding (a downhill, a landing,
  feathering), and the port's is a clean standstill on the flat. The port's 0 to 60 of 1.33 s is still
  under half the original's quickest.
- **The turn comparisons are loose.** The original's held-stick stretches are 0.3 to 1.8 s long, with the
  gas not always full and the stick not always at 1.0. The port's are 1 s windows starting 0.33 s after
  a hard-over stick, with the gas pinned and the speed changing through them.
- **Lip runs are straight approaches.** Each lip was ridden with a straight run-up of 30–120 ft from a
  standstill along the lip's own heading. The original came in on its own line, so the take-off vy
  difference may be partly approach and suspension state rather than the lip.
- **The fixture minute is at DT 1/60, and the captures and the shell are at 1/30.** The leg thresholds
  compare seconds against seconds, so this matters only as far as the step size changes the ride.
- **The raised turn, jump and landing thresholds are floors at what the fixture carries**, not the
  original's numbers. The table above says how far below the original each one is, and P26 is the task
  that would assert the original's.
- **The recording2 capture is the acceptance spec's columns only.** C6 is still open for the trick and air
  columns and the index. The capture is not committed: it is game data under context/.

## Next

The gap tasks, P20–P26. P25 is the one-line start; P22's R6 question gates any distance comparison.
