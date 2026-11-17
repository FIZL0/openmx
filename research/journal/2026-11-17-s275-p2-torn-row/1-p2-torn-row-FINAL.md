# 1 — p2 torn row (FINAL)

Task S275. 2026-11-17.

## What I did

- The brief said P2 is seeded from its chassis only (`standing_p2`). That is true of the Sim plane lane
  (`fly`), but the joust pin is `fly_two`'s, which already seeds P2 whole with `seeded_rider` (chassis,
  both wheels, engine, balance, air, levers). So no seeding was missing.
- Printed P2's port chassis beside the capture from 137810. Every row reads a port state bit for bit,
  except row 137829.
- Row 137829 is `[vel after tick 137827, centre 458f6d8d 43088743 455bd5f0]`. The centre is exactly the
  one chassis integrate rebuilds first in tick 137828: the transform after 137827 applied to the local
  centre (ride.rs's `orientation_point`). So the row was read inside integrate (memory note "2p torn
  rows are integrate's rebuild").
- tests/c16_fly.rs: `Tick::p2_rebuilt` (that centre, after each `fly_two` tick, by the new
  `rebuilt_centre`) and `p2_held` takes `[vel, rebuilt]` as a reading inside the tick, through
  `held_in` (the plane's T8 path). `held_by` had no other caller and is gone.
- JOUSTS 137810: P2 17 → 90 (of 90). Others unchanged. A scan of the three joust runs found 137829
  the only row matched this way.
- The C16 capture, tape and s272 images were missing from the shared context. Copied them in from
  `openmx-slots/kept-context/c16` (new files only).
- Took nothing from ../opentennis: this is a test-side capture-row rule specific to this game's chassis.

## What the original does

`chassis_integrate` first writes the centre (+0x60) rebuilt from the transform, then integrates and
writes the final centre later. A PINE read between the two sees the old velocity and the rebuilt centre.

## How it was proved

A PINE recording compared frame by frame: `context/capture/c16/fly.csv` against `fly_two` from
`s272-137810.ram`, bit for bit on P2's chassis velocity and centre for all 90 ticks.
`cargo test --release -p game-sim --test c16_fly`: 11 passed, 6 ignored.

## Not verified / not 1:1

- The seed's own rebuilt centre is not computed (the seed stands for it in `p2_held`'s inside list):
  no row here reads the image's frame mid-integrate, so it was not needed.
- The rebuilt centre assumes one chassis substep per tick (it uses the transform at the tick's end).
  True for the row it matched; a multi-substep tick's mid-frame row would not match and would read as
  a parting, not a false hold.
- The s272 images 137854..137858 are not in the shared context, so the f30/f32 image tests in
  c16_fly skip here; this task does not touch them.

## Next

None.
