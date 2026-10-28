# 1 — 2p whole rider replay (FINAL)

Task S174. 2026-10-28.

## What I did

- New test binary `crates/game-sim/tests/two_p_rider.rs`. It builds both riders the way the shell does:
  Race500P from the disc, the bike's collision files from `free09_ss_2.res`, `fifty` on, and no start line.
  The wheel mounts come off the image (each wheel's `+0xa0`). Each rider is then seeded off C10's RAM image
  of the tick before: chassis and node, both wheels, engine, clutch, balance, air and levers. The seeders are
  copied from `tests/replay.rs`'s `SaveState` and keyed on S157's player chain. A `Sim` is built from the
  pair (`with_rider` + `add_player`), and one `Sim::step_all` runs on neutral pads through the split free09
  (`Wooded<Place>`: terrain, the scene's props and the ecosystem's trunks).
  `Rider::frame_bumped` puts the wreck list together itself: ground entries, the head ray against the
  other rider (`hits_riders`, because the body rays searched), the cars' entries, then the body-ray scenery,
  all after the chassis' sweep back. The hit clause runs on that list.
- `the_whole_rider_wrecks_p2_on_the_trunk_as_the_original`: covers all four of S157's world contacts.
  - 130416→130417 and 130895→130896: port `[None, Some(Wreck(6))]`, which matches the next image's
    `+0x748` `[0, 6]`.
  - 130412 (P2's nose ray on the trunk) and 131482 (P1's nose ray in the ground): `[None, None]`, as the
    original has it. 131483's image confirms the second one. C10 saved no image of 130413, so the
    original's `[0, 0]` there comes from S157's interpreter tick.
- `without_the_trees_nothing_wrecks_p2`: the same two wreck ticks with the ecosystem removed from the world
  (`world.world`) wreck nobody. So the trunk causes the verdict, not the seeded attitude, loads or ground.
- I checked that it does not skip: a throwaway panicking test printed
  `[(Some([None, Some(Wreck(6))]), Some([None, None])), …]` for both images in s3. The memory note on
  S157's skip is about `c10-130413.ram` only; 130412/130416/130417/130895/130896/131482/131483 are all in
  s3's `context/states/c10`.
- Nothing taken from opentennis: it has no bike or wreck test.

## What the original does

S157 has the full account. In the original, the collision node's tick resolves each rider's record list,
and the bike's update runs the wreck test on that list. At 130416 and 130895 one of P2's shoulder rays
(reach 4.58 ft, above the 50cc limit of 4) meets a trunk, and the wreck reason becomes 6 (scenery).

## How it was proved

Save-state seeding plus the port's whole frame, compared with the original's next RAM image (the wreck
reason words of both riders), on 4 ticks, plus a 2-tick control without trees. Tests are in
`crates/game-sim/tests/two_p_rider.rs`.

## Not verified / not 1:1

- **Only the verdict is compared, not the composed list entry by entry.** `Rider::frame_bumped` does not
  expose the list it builds. S157 compared the body rays' entries bit for bit, but from the image's node
  with no sweep back. This test does run the sweep back, but it only shows the verdict agrees.
- **The chassis after the tick is not compared to the next image.** The pads are neutral
  (`Controls::default()`), not the frame's real input. S157 found that P1's drive moves with the frame's
  input even inside the node tick. The rider's tricks, motion, score and state-machine clocks start at
  `Rider::new`'s defaults, not the image's. Only the wreck reasons were checked.
- **The cars' part of the list is empty by construction.** `Sim.traffic` is empty here, and the 2p state
  has no truck, rider or prop contact at these ticks (C10's index; S157 checked that the original's entries
  have no mover). So the cars clause is in the path but never contributes.
- **The head sweep against the other rider finds nothing at these ticks**, as in the original, where
  neither list has a rider entry. It is exercised, not proved, here.
- **The wheel probe record, the tire flick ring and the d-pad axes are not seeded** (replay.rs does not seed
  them either). They do not feed the wreck list.
- The original's `[0, 0]` after 130412 is S157's interpreter tick, not a saved image, because
  `c10-130413.ram` does not exist in any slot.

## Next

Nothing for S174. A later task could compare the chassis after the tick to the next image by seeding the
frame's pad from `@pad0_control_state` / the 2p pad state, and rider/state-machine fields as well.
