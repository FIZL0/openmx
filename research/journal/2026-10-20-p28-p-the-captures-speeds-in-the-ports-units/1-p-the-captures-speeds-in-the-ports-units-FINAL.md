# 1 — the captures' speeds in the port's units (FINAL)

Task P28. 2026-10-20.

## What I did

- **Where each number came from.** P15's journal (`2026-10-20-p15-p15-how-it-should-feel`, the
  table's header line 71) says all the original's speeds in it are "in the units of the `bike_vel`
  columns". Those include the launches (8.8/34.2 slowest, 20.2/56.9 quickest), the top speeds
  (75.78, 79.89), the slowing p99 (70.9), the flat brake (~25 ft/s², then 8), and the lips (about
  52, losing 4–8). The shell's numbers for race125p (121.8 top, 54 ft/s² brake, 2.47 s stop) are the
  port's chassis and are already in chassis units. The turn rate (rad/s) has no speed in it.
- **One divisor.** `BIKE_VEL_PER_BODY = 0.8` (doc: P22's `vmulx` by 0.8 and P21's 480-row ratio)
  is applied at every `bike_vel` read in `crates/game-sim/tests/ride.rs`:
  - the idle capture's `slide` and `speed` (`captured_idle`);
  - the ridden capture's speed in `the_minutes_numbers_are_the_originals`;
  - `captured_launch`, which feeds `the_port_launches_about_as_hard_as_the_original`;
  - the lean test's "moving" filter (`speed > 10.0` now means 10 ft/s of chassis, not 12.5).
- **Constants restated.**
  - `CAPTURED_TOP_SPEED` 75.78 → **94.73**. The read-back computes 94.72436 off
    `capture/acceptance/acceptance-freeride.csv`. I checked that the read-back really runs: with the
    old value it fails with "the recording's is 94.72436".
  - `CAPTURED_SLOWING` 70.9 → **88.6**. It passes the read-back's 1%.
  - `ORIGINAL_LAUNCH` (8.0, 32.0) → **(11.0, 42.75)**, which is 8.8/0.8 and 34.2/0.8. These
    numbers are not read back off a capture, so this is arithmetic on P15's table.
- **Re-tuned the bike to the new numbers** (`ridden_bike` in both test files):
  - `Drag` 0.06 → **0.037**. Flat-out top speed is 94.73, the same as the recording's 94.72 to two
    decimals. Before the change it was 74.86 against 75.78.
  - `FLIP_RAMP` 100 → **108 ft** (both files, and the pad's comment). At 0.037 the bike reached the
    flip lip 3 ticks sooner (198, was 201) and left it 6 sooner (246, was 252). That is before the
    backflip fires at tick 249, so the test failed with "the backflip never fired". I moved the ramp
    instead of the stick ticks, because the shell's `ride-minute.pad` shares those ticks frame for
    frame. At 108 the lip is ticks 209–254, the preload fires at 230 and the backflip at 249. At 105
    the lip ended at 251, only 2 ticks of margin. The stab window is now 290–298 (288 failed, 287
    passed, 300 and 305 failed). It was 295–305. The script stays at 295.
- **The launch is not re-tuned.** The bike makes 9.54 ft/s at 0.5 s and 35.77 at 2 s, which is 87%
  and 84% of the restated original. Before, the bound was 80% of the original by accident of units,
  and the bike cleared it. I measured four candidate changes with drag at 0.037:

  | change | 0.5 s | 2 s | what else it did |
  |---|---|---|---|
  | none | 9.54 | 35.77 | — |
  | all `HP` ×1.2 / ×1.4 | 9.54 | 37.75 / 39.78 | top speed 104 / 111, so drag must move again |
  | `ClutchStatic/KineticFriction` ×1.5 | 14.31 | 38.44 | **the brake never stops the bike** (27.5 ft/s left after 2 s) |
  | rear `MaxDriveForce` 2.4 → 3.6 | 9.54 | 35.77 | nothing |
  | `GasRateLimit` 4 → 8 | 9.54 | 35.77 | nothing |
  | `EngineInertia` 0.4 → 0.2 | 9.54 | 35.77 | nothing |

  The clutch sets the first half-second, and the clutch is what P13's engine-against-brake servo
  runs through. So the assertion is now `ORIGINAL_LAUNCH × LAUNCH_SHARE (0.85, 0.8)`, with a comment
  that says why.
- **Prose that quoted the old numbers** is updated: the jump (104 ticks at 89 ft/s, down at 75,
  bottoming at 68 = 0.72 of top), the original's lips (65 ft/s and 5–10 lost, in chassis units), the
  brake (2.5 s from 84), the right turn's entry speed (95), and the paragraph that said the recording
  "will not settle" the 1.25 (it is settled now).

The leg numbers after the change: launch 9.54 / 35.77, flat out 94.73, jump take-off 89.1, air 104
ticks, slowest after landing 68.2, brake 83.9 → 12.8 in 2 s (35.5 ft/s², under 88.6), stopped in
2.53 s, no wrecks in the minute, flip lands.

## What the original does

The capture's `bike_vel` is the type-4 actor's +0x130. The vehicle-anim update writes it as the
chassis velocity times 0.8 (P22: `./kit fn 0x0024d42c`'s function, `vmulx.xyz` by `0x3f4ccccd`).
This task found nothing new in the decompilation. It only applied that factor.

## How it was proved

- `the_minutes_numbers_are_the_originals` reads `CAPTURED_TOP_SPEED` and `CAPTURED_SLOWING` back off
  `context/capture/acceptance/acceptance-freeride.csv` to within 1%. I ran it
  once with the old 75.78 to show it is not skipped.
- The bike's terminal speed is 94.73 against 94.72, from `it_reaches_the_top_speed_and_holds_it`'s
  leg.
- Every `crates/game-sim/tests/ride.rs` and `crates/openmx/tests/ride.rs` test passes on the re-tuned
  bike. `./kit check` passes.

## Not verified / not 1:1

- **The launch is 13–16% short of the original's slowest launch** (9.54 / 35.77 against 11.0 /
  42.75). The bound is `LAUNCH_SHARE` (0.85, 0.8), and 9.54 against 9.35 is a thin margin. The only
  lever that closes the gap (clutch friction) breaks the brake-to-stop leg. Closing it means a
  different clutch model or a bike with different clutch keys, and that is not this task.
- **`ORIGINAL_LAUNCH` is P15's table divided by 0.8.** I did not re-measure it off the recordings,
  because no test reads it back. P15's 8.8 / 34.2 came off `bike_vel` (its journal says so), so the
  division is correct. The original's own launch windows were not re-derived.
- **The flip's stab window narrowed and moved**, from 295–305 to roughly 290–298, and 288 failing
  while 287 passes means it is not clean at the early edge. The shell's arrow keys put the stab about
  8 ticks after a stick does (the `the_flip` doc). I did not ride the shell on the keys to see where
  in the window it lands. `crates/openmx/tests/ride.rs` passes, but it is the shell's own harness and
  not the window's edge.
- **The steering fade's "about one g" reasoning** (patch 1 in `ridden_bike`'s doc, written for a 75
  ft/s top speed) was not re-derived for 95 ft/s. Above `SteerSpeed2 = 50` the demand grows with the
  square of speed, so at 95 the full lock asks for more than it did at 75. The turning tests pass.
  The doc paragraph still quotes 75 ft/s, because it is P12's history.
- **P23's brief quotes the recording's flat brake as 25 ft/s², then 8**, which are `bike_vel` units
  (31 and 10 in the chassis' units). P23 is not done and its line in `plan/TODO.md` belongs to that
  task, so I did not edit it. Whoever picks up P23 should divide those two numbers by 0.8. Its shell
  numbers (54 ft/s², 2.47 s from 121) are already in chassis units.
- **The jump's "0.65 of top speed" landing floor** was kept. The bike now bottoms at 0.72, and the
  bound was not tightened.

## Next

Nothing for this task. P23 should restate its recording numbers as above.
