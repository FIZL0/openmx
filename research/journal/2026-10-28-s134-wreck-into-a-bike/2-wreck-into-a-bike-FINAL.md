# 2 — a wreck bike into a rider's bike, replayed bit-exact (FINAL)

2026-11-21. Follows [1-wreck-into-a-bike-PART.md](1-wreck-into-a-bike-PART.md).

## The capture: one we had already

No new emulator run was needed. S93's short split-screen ride (`splitscreen2_shortcolision.p2m2`,
image `context/states/s93/s93-short-5997.ram`, capture `context/capture/split/splitscreen2.csv`, every
tick 5765..6152) has it: P2 T-bones P1 and wrecks at 5998, and P2's wreck bike tumbles on into P1's
bike. S93's journal noted it ("from 5999 P1 also hits P2's wreck bike as it tumbles past"). Two human
pads, so the struck bike does not dodge, as free09's AI riders did in part 1.

Copied from `../kept-context/s4` into this slot's `context/` (git-ignored): the three s93 images and
`capture/split/*`.

## The original, run whole

The test runs the original's frames whole in the interpreter from the image, as S176 does for C10's 2p
ride: the capture's two pads laid into libpad's buffers, the device update, the markers, the scene
root's simulate. From 5999 on, both riders' chassis centres are the capture's to the bit, every frame
to 6040. (5998, the first frame run, is not compared: that row is not the image's next frame to the bit.)

What it shows (addresses in `research/oracle.txt`, S134):

- P2's wreck bike's record lists its own ray hits on P1's bike: kind-0 entries naming P1's record on
  5999 (2), 6000 (2) and 6001 (1). P1's record takes the kind-1 mirrors naming the wreck bike's record:
  5 in all.
- Hooking the resolve shows the order within a frame. The wreck bike's four substeps run first, then
  P1's record's. P1's chassis is the previous frame's all through the wreck bike's step.
- The wreck bike's resolve leaves P1's chassis velocity as it was.

## The port

`WreckBike::tick_ridden` (crates/game-sim/src/wreck_bike.rs) is `tick_among` with the riders' bikes in
its world (`Riders`: the bikes with `bikef.col`, each rider's chassis as a `contact::Vehicle`, each
rider's record's collision object):

- The search is the plane's own: `Scene::flown(…, Some(bikes), …).owned(velocities)`. A hit on bike
  `k` names that rider's record, `1 + k` among the records handed to the search, which takes the mirror.
- The bike's entries are renumbered onto the riders' others, after any props struck on it this tick.
  Each rider is the other side, `Side::Vehicle`, as the plane's force listener has it.
- The answer is the mirrors each rider's record took, for the caller to file.

Existing callers are unchanged. `tick_among` still hands in no bikes.

## How it was proved

`a_wreck_bike_into_a_riders_bike_replays_bit_exact` (crates/game-sim/tests/wreck_bike.rs) seeds the
port's wreck bike from the interpreter's state after 5998. It ticks the bike to 6040 with both riders
as the frame before left them. Every tick:

- The body is the original's to the bit.
- Its entries name P1's record where the original's do.
- P1's record takes the original's mirrors, normal and point to the bit.
- The chassis handed in comes back unchanged.

The same bike ticked by `tick_among`, without the bikes, parts at 5999. Ticks striking a bike: 3. Mirrors
filed: 5.

## Not verified / not 1:1

- No game loop calls `tick_ridden` yet. The 2p ride's wreck still ticks with `tick_among`, and the
  mirrors are not filed on the struck rider's list: follow-up S502.
- Toys (0x48): path cars are in the moving list already (S171). None was near this wreck, so the two
  together are untested. 0x200020 objects (the gates) are still not handed in: follow-up S503.
- The test has no prop striking the wreck bike, so the riders' renumbering after struck props is not
  exercised.
- Only one wreck: a wreck bike ray-striking a riding bike. A bike's sweep hit, a resolve that pushes the
  chassis, and the long ride's slow rub of P2 on P1's wreck bike (3414) are not checked.
- Frame 5998's interpreter row is not compared with the capture's.
