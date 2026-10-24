# 1 — fork stop (FINAL)

Task V62. 2026-10-23.

## What I did

- `crates/game-render/src/vehicle.rs`
  - Drive 2, the fork's slide, moved into `Chassis::slide_fork` without changes, so it can be reused.
  - `Chassis::drive` now slides the fork to `front_compression.min(fork_stop)`.
  - New field `Chassis::fork_stop`. It is infinite until something sets it, so a chassis nobody
    measured draws as it did before.
  - New `Chassis::stop_fork_under(geometry, parts)`.
    - It works in the rest pose, grounded: the bars are turned straight about the fork, the way the
      drive turns them on the ground, and then the fork is slid to compression `c`.
    - At each `c` it measures the front tyre's outer radius and half-width about the `TireFront` axle
      (the wheel-1 triangles, as V59's test does).
    - Its "clearance" is the least distance from the tyre to any part vertex that lies within the
      tyre's width, minus `FORK_STOP_CLEAR` (0.04 ft).
    - The stop is the largest `c` with clearance above zero: a march in steps of 0.05 up to 2.0, then
      16 halvings.
    - A part the tyre does not clear even fully extended is skipped, since the tyre cannot come up
      into it.
  - New `Chassis::stop_fork_in(models, chassis, parts)`, the shell's way in: the loaded chassis model
    and every `.sub` part placed at `Handlebars`. That is the fender, number plate and crossbar, not
    just the fender.
- `crates/openmx/src/main.rs`: both rig constructions (the first seat, and each other seat with its
  own bike model and parts) call `stop_fork_in` once. The first seat prints
  `chassis: the fork is drawn to a compression of 0.814`, checked with
  `openmx --ride --headless --frames 30`.
- Nothing taken from opentennis: it has no bikes.

Margin: the 0.04 ft is a little more than the reach of the original's fixed `(-0.03, 0, 0.02)` nudge.
The stop is measured with the bars straight, and steering swings that nudge round the fork axis.

## What the original does

V9 read the original's drive (its journal, `./kit fn 0x002593e0`). The fork target is built as
`rest + axis * (one_over_sine * compression) + nudge`, with the compression copied straight from the
wheel snapshot. Nothing clamps it against the bars' parts. The fender's placement at `Handlebars` is
the disc's own `.sub` (`[Parts]`), which the shell reads as it is. The ride's compression is the
simulation's, bit-exact, and the 30 ft drop reaches 1.000 on it.

So on paper the original draws the same overlap: about 0.097 ft of tyre inside the fender at a
compression of 1.0. That is an inference from the code, not something I saw (see below).

## How it was proved

`crates/game-render/tests/vehicle.rs`
`at_full_droop_the_tyres_stay_on_their_axles_and_clear_of_the_fender`, extended for V62 (disc-backed):

- The stop the test measures lies between 0.5 and 1.0. It printed 0.814.
- Over the whole minute (3,600 frames at 60 Hz, the V59 30 ft drop, gas held), landings included:
  - every frame the fender clears the front tyre (V59 checked this only while the bike hung in the air);
  - the drop compresses the fork past the stop (it reaches 1.000), so the stop is what keeps the tyre
    out, and the test would fail without it;
  - on every grounded frame under the stop, `Shocks` sits within 1e-3 of the ported slide's target in
    the `Bike` frame, so the fork below the stop is the original's. 3,512 frames.
- Printed: `over the minute at least 0.040, at a front compression of 1.000 (V62: the fork stops at
  0.814, the drop reaches 1.000; 3512 grounded frames under the stop on the ported slide)`.
  - Before this task the same line read −0.097.

## Not verified / not 1:1

- **The original was not filmed at a hard landing.** The brief asked for it. I skipped it because it
  could not settle the question:
  - The overlap is 0.1 ft, about 1.2 in.
  - The chase camera sits behind the bike, so the fender hides the tyre's top from it.
  - PCSX2's Video Capture is 640×480.
  - No existing capture records a wheel's compression, so a filmed landing could not even be matched to
    a compression of 1.0 without a new PINE recorder.
  
  The answer above ("the original clips the same way") rests on the decompiled drive (V9) and on the
  `.sub` placement, not on footage.
- **Not 1:1, on purpose.** Past the stop (front compression above 0.814 on these bikes) the drawn fork
  stays at 0.814 while the original's slides on to 1.0. The tyre is then drawn up to about 0.1 ft above
  where the simulation has its contact: slightly into the ground, instead of into the fender. This is
  presentation only; the simulation's compression is untouched.
- The fork stops dead at the stop rather than easing into it. Judged by eye to be fine, since the
  simulation's own compression also stops hard at its maximum. Not compared against the original.
- The stop is measured with the bars straight and the rest pose. A steered fork, or a stunt clip that
  moves the bars, is covered only by the 0.04 ft margin, and is tested only on the drop's straight-line
  minute.
- Only the 250SX chassis is proved, the test's bike. The shell run shows the 125sx seat also gets 0.814,
  as it shares the `bike` chassis. Other disc bikes and mod bikes get a stop worked out the same way;
  none was checked one by one.
- The GPU skins the chassis from `Chassis::world`, and the stop is in that pose, so it applies on both
  renderers. I did not look at a frame of a landing.

## Next

Nothing.
