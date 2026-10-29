# 1 — seeded truck roll (FINAL)

Task S182. 2026-10-29.

## What I did

- The port was right; the harness was wrong. `two_p_truck.rs`'s `roll` put a **fresh**
  `Sim::ride` rider where image 132230's chassis node is and gave it only the chassis velocity. A
  fresh rider's clutch launch ramp (`mv::clutch`, move.rs) has not run yet: timer 0 and cofg weight 0.
  With the lever out (`held.clutch` 0) the ramp starts on the first tick, setting scale start 3 and
  cofg weight 1, and for 1.5 s it shifts `ch.cofg.z` back by `ClutchCofGTravelBack`. The idle rule
  cuts the drive below IDLE_SPEED, so the shifted centre of gravity is the ramp's only effect here.
  It held the bike to the deck: 0.2 ft back, where the original goes 0.68 ft. In the image the ramp
  is long over: timer 1.5001, start 3.0, weight 1.0, torque scale 1.0.
- Bisection with the seeded rider (back in ft, drag / no drag): whole seed 0.747 / 0.405; without
  the engine/clutch/lever block 0.395 / 0.041; without only the three ramp fields 0.347 / −0.008.
  Seeding the engine struct, balance, air, stick, lean and brake levers changed nothing that showed.
- `roll` now seeds P1 off the image with S174's `Image` seeder, copied from tests/two_p_rider.rs:
  mounts, chassis, wheels, engine, clutch ramp, balance, air and levers. It drives each tick `t` on
  capture row `t`'s `pad0_a` report and steps at row `t + 1`'s `dt`. The 2p state's dt alternates
  between 0.016666654 and 0.016699988. A constant dt, or any other row offset, parts at the first tick.
  A pad lag of 1 parts too (the brake is let go at 132231).
- New test `the_roll_back_is_the_originals`. `the_port_keeps_p1_on_the_deck` now also asserts the
  back distance and that no-drag rolls back less. Drag is about half of the roll: 0.681 ft with drag,
  0.370 without. `print_the_roll` prints the capture's deck velocity beside the port's.

## What the original does

On car 3's deck at ~40 ft/s, with the brake let go and nothing else held, P1's bike rolls back
~0.68 ft over 132231..132266. Half of that is the bike's air drag (it reads speed over the world),
and the other half is the deck's slope plus the tire/suspension response. The clutch ramp is not in
play because it was spent long before. Nothing in the port changes.

## How it was proved

A bit-exact replay fixture: `crates/game-sim/tests/two_p_truck.rs`
`the_roll_back_is_the_originals`. From S159's image 132230, 59 ticks on the C12 truck capture's pads
and dts. Every tick's chassis velocity and centre are bit-equal to capture row `t + 2`'s
`p1_ch_vel` / `p1_ch_centre`. After the last tick, velocity, centre and orientation are bit-equal
to image 132289's chassis `+0x260/+0x60/+0x1a0` (xyz lanes). The window includes P1's control 7 at
132267..132273.

## Not verified / not 1:1

- That the clutch ramp's cofg travel is *what* held the fresh rider is shown only by bisection over
  seeded fields (skipping the three ramp fields drops the roll to the fresh rider's). The lever arm
  of the cofg shift against the deck was not isolated term by term.
- The fixtures exist only in some slots: the S159 images, `context/capture/2p/truck.csv` (s1/s3; s2
  has a symlink to s1's), and the disc. Without them the three roll tests return early and read as
  passing.
- The orientation is compared on the xyz lanes only (w as well would need the image's quat lane
  order confirmed; the xyz lanes already pin it).
- Only one window (132230→132289) is driven; the rest of the truck ride (the reverse, the ride off
  the front) is not replayed by this test.

## Next

Nothing for S182. Other harnesses that set a fresh `Sim::ride` rider down on an image and compare
its motion will have the same unspent clutch ramp. They should seed the rider (S174's `Image`) instead.
