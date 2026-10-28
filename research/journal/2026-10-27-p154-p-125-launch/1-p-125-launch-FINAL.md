# 1 — p 125 launch (FINAL)

Task P154. 2026-10-27.

## What I did

- I put the port's launch beside P21's flat-throttle capture, which records the engine (gear, rpm,
  road rpm, torque, wheel torque, drive spin, throttle and clutch), and found the cause. It is the
  **start state**: the clutch's launch ramp. The drive force, the gearing and the clutch model are fine.
  - S100's port run built the bike with `Rider::new` and gave it the gas on its next frame. The
    ramp's timer is 0 then, so `mv::clutch` runs the ramp: `MaxClutchTorqueScale` (3.0 on race125p)
    times the engine's torque from a standstill, fading to 1 over `ClutchTorqueScaleTime` (1.5 s).
    The port's torque at 4500 rpm was 60–62 against the original's 24.028, and in that run the wheel
    spun up to the rev limiter.
  - Both of the original's captures start from states that had stood or been ridden far longer
    than 1.5 s with the lever out. Their ramp is spent and the engine drives at 1x.
- `crates/game-sim/tests/ride.rs` `the_125_launches_as_the_original` (new), with `launch_125` and
  `Pull`. The disc's 125 stands 2 s on a Flat floor with the lever out, so the ramp runs out, then
  rides 70 ticks of gas. The test compares:
  - against P21's capture (`context/capture/p21/flat-throttle.csv`; the port's gas tick `t` is row `t + 1`):
    the speed through first gear, the wheel torque on the three slip-out ticks, the revs held at
    MinRPM on ticks 6–25, and the shift into second;
  - against S100's `nowreck.csv`: the speed gained from tick 10 to 50;
  - the cause itself: the same settle with the lever held in leaves the ramp unspent, and that bike
    is more than 1.4x as fast at tick 20.
- `crates/game-sim/tests/eco.rs` `a_free09_tree_against_the_original`: the port's bike now stands
  60 ticks before the gas, as the captured one had. It now reaches the trunk on tick 59 at 57.85 ft/s,
  where the original arrives on tick 61 at 58.16. Before, it was tick 51 at 64.6.
- `crates/game-sim/src/ride.rs`: `Ride::new`'s doc says the ramp starts unspent, as the original's
  constructor leaves it. No simulation code changed.
- `research/oracle.txt`: a P154 block listing the ramp timer's writers.
- This slot's `context/capture/p21/` is a copy of s3's P21 capture (git-ignored).

opentennis: nothing applies; this is the bike's engine.

## What the original does

- `./kit fn 0x002572f0` (bike_clutch) works off the timer at bike+0x6c0:
  - The lever (+0x644) held in with the gas (+0x63c) pinned rearms the timer to 0. That is the
    clutch launch.
  - Otherwise, with the lever out and the timer at or under +0x6c8 (ClutchTorqueScaleTime), the
    ramp runs. **The gas plays no part in that clause**, so a bike that stands with the lever out
    spends the ramp in 1.5 s.
- The bike constructor (`./kit fn 0x0024f420`) zeroes +0x6c0. I scanned an objdump of boot.elf
  for stores to offset 0x6c0. The only stores that land on a bike are the constructor's and
  bike_clutch's own. The rest are other objects' constructors (list in oracle.txt). No put-back
  resets the timer.
- P21's launch, by the capture's lanes:
  - Ticks 1–3: the clutch (the rate-limited +0x65c) goes 1 → 0.666 → 0.333 → 0 while the throttle
    opens 0.333 a tick. The wheel gets 379.330, 758.660, 1137.990, which is the slipping clutch's
    kinetic torque.
  - The clutch locks on tick 5. From there the engine is held at MinRPM 4500 (the road turns it
    slower) and makes 24.028 at the wheel ×18.968 = 455.771. That gives an even 0.96–0.97 ft/s a tick.
  - Past 4500 road rpm (tick 32) the revs follow the road and the curve. The shift into second
    comes on tick 58.

## How it was proved

All of it is in `the_125_launches_as_the_original`, against two live PINE captures (P21's 480 rows
and S100's 300 rows). The run I made printed:

| | the original | the port |
|---|---|---|
| tick 1 / 2 / 3 at the wheel | 379.330 / 758.660 / 1137.990 | 379.330 / 758.660 / 1137.990 (asserted < 0.01) |
| tick 6 / 20 at the wheel | 455.771 / 455.771 | 455.771 / 455.771 |
| revs, ticks 6–25 | 4500 | 4500 (asserted) |
| tick 10 / 20 / 30 / 50 speed | 10.573 / 20.239 / 29.773 / 49.645 | 10.507 / 20.174 / 29.709 / 49.584 |
| into second | tick 58 | tick 58 (asserted ±1) |
| S100: gain from tick 10 to 50 | 0.9666 ft/s a tick | 0.9724 (asserted within 2%) |
| ramp unspent, tick 20 | — | 36.54 ft/s against 20.17 (asserted > 1.4x) |

Through first gear the two speeds never part by more than 0.202 ft/s, at tick 53. The bound is 0.25.

`./kit check -- -p game-sim --test ride -- the_125_launches` and `--test eco -- a_free09_tree_against`
both pass. I read the printed output in the check log to confirm the tests ran and did not skip.

## Not verified / not 1:1

- **A freshly built bike's launch in the original was not captured.** That it launches on the ramp
  (3x at a standstill) is read from the decompilation: the constructor zeroes the timer, and
  bike_clutch runs the ramp while the lever is out. Getting a fresh spawn under PINE (a menu start,
  gas held from the first frame) was not attempted. The port follows the same rules, so it gives
  the same answer, but no live data checks that answer.
- **The speeds sit about 0.06 ft/s under P21's from tick 1 on**, peaking at 0.20 near tick 53. The
  wheel torque and revs agree to print precision, so this is not the drive. Likely causes are the
  lane's slight grade (cofg y climbs about 0.2 ft over the first 30 ticks) and the start: the port's
  settled bike creeps at about −0.04 ft/s, the captured one at 0.001. I did not ride the port on
  free09's real terrain to separate the two.
- **Tick 5's wheel torque** is 443.749 in the original and 444.160 in the port: the tick the clutch
  locks. Not asserted, not chased.
- **The surface is the Flat floor's 0**, as in P21's top-speed test. Which surface id free09's lane
  is, I did not read. The first-gear match suggests the drive is not traction-bound there.
- **S100's run is compared by rate only.** Its rows read at mixed phases (S100's journal), so it sits
  about a tick behind P21's. The eco test's printed run-up, port against original by row, still
  shows that 1.6 ft/s phase gap.
- **The eco test's trunk stop** is now 4.02 ft off the trunk's axis, against the original's 3.13
  and inside the test's 2.5–4.5 band. The start is `Rider::new` at the capture's x, z and heading,
  not the captured state, so the approach line is not the original's.
- The comparison stops at the shift into second. Second gear onwards is P21's top-speed test's
  ground, and this test does not assert it.

## Next

None for P154. S124 (the trunk impact) now rides into the trunk at nearly the original's speed.
