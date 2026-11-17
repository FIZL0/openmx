# 1 — untamed bike step (FINAL)

Task MD93e. 2026-11-17.

## What I did

- `ride::Controls` has a new field, `preload: Option<F>`. It holds Untamed's analogue preload axis, and `None` means
  nothing is bound to it.
- The pad read (`Controls::through` / `through_stick`) leaves it `None`. MX Unleashed's binding table has no such
  command, and Untamed ships with action 0x53 unbound (# MD93f, # R30 in research/oracle.txt).
- `Controls::with_preload(pad, axis, range)` binds a pad axis to it, read through `range`.
- `Ride::frame` passes `c.preload` to `handling::untamed::preload` on `Handling::Untamed`. Before this it passed
  `None`. Nothing changes on other handlings: they never read the field.
- research/untamed-machines.md's preload row now names the field.
- Filed P367 (low): a controls-menu entry so the player can pick an axis for it.
- I took nothing from ../opentennis. It has no bike preload.

## What the original does

- The bike's pad reader copies pad word 0xe5 into bike +0x12b4. It also sets +0x12c8 = 1 when the binding lookup
  for action 0x53 is not -1.
- Word 0xe5 is the device's axis value for whatever 0x53 is bound to (R30).
- `./kit fn 0x004fe360` (preload) reads both values. MD93 ported and proved that function with the axis as an
  argument.

## How it was proved

- The preload itself was proved by MD93 (tests/untamed_reworked.rs, against the image).
- This task only adds the hand-in. Two tests in crates/game-sim/tests/ride.rs cover it:
  - `an_untamed_bike_fires_its_bound_analogue_preload_on_the_lip`: the disc's 125 on Untamed handling rides up
    FLIP_RAMP with the stick centred. With the axis bound at 1.0, the preload fires as the lip arrives (on a lip
    tick, at most one tick after the first lip row) with force y == MaxPreload, bit for bit. It never fires when the
    axis is unbound, when it is at 0.001 (the dead zone), or on the original handling.
  - `the_pad_read_leaves_the_preload_unbound_until_one_is_bound`: the pad read gives `None`, and `with_preload`
    gives the axis's value.

## Not verified / not 1:1

- **How Untamed's device scales the bound axis.** SLUS-21701 reads the raw axis word (after its own dead-zone remap,
  0x00116e90). `with_preload` reads it through a range the caller picks. That remap was not compared with
  `input::AxisRange`, because no caller binds an axis yet (P367 will). The shipped game binds nothing, so a ride on
  the shipped bindings is exact.
- **The second device.** Untamed lets a second device's raw axes win on words 0xdf/0xe0 only. Word 0xe5 is not
  touched (R30), so nothing is missing there. But that comes from reading the code, not from a capture.
- **A bound-axis ride against a live Untamed run.** No capture with 0x53 bound exists. The shipped game cannot bind
  it from its menus as far as we know, so the test checks the port's own wiring, not a recording.
- **The fire tick in the ride test.** The test reads the lip off the end of the tick, and the preload reads it in a
  substep. So the test allows the fire to land one tick after the first lip row instead of pinning the exact tick.

## Next

- Nothing for this task. P367 lets the player bind the axis.
