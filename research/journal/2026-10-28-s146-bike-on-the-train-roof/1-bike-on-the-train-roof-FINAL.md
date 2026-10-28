# 1 — bike on the train roof (FINAL)

Task S146. 2026-10-28.

## What I did

- **Made the roof state in the original.** I wrote `research/s146/roof.py` (S129's `tele.py` teleport, run on copy 2). It:
  - loads S129's along state (slot 8) with both brakes held;
  - runs 16 frames;
  - moves the bike 2.5 ft over the middle of the tenth path car's roof. That car is S81's order 9, the train's second `train_car_06` boxcar; its roof is 16.6 ft over its node.
  - sets the chassis velocity to the wagon's own: node position minus car +0x310, over dt, which gave 55.157 ft/s;
  - rides 30 frames and saves into slot 13.

  Command: `./kit pcsx2 with -- python3 -B research/s146/roof.py <pid> 8 0 30 13`.
- **Recorded it** at quarter speed with S129's spec and braked pad, after `resume_if_paused.py`: `./kit record watch/s129-train.toml --state 13 --pad watch/pads/s129-braked.pad -n 270 -o context/capture/s146/roof.csv`. It gave 270 rows with no torn reads and no gaps. The state's RAM is in `context/states/s146-roof.eeMemory.bin`. All of this is in s2's context only.
- **Added `tests/replay.rs` `a_bike_parked_on_the_trains_roof_rides_it_as_the_original_does`.** It replays the image among its 21 seeded cars and checks:
  - every chassis, wheel, engine and slip lane, plus the train clock, on every tick;
  - each bike's place on the wagon's deck;
  - a control run with air drag zeroed.

  It sits next to S129's test because the save-state, car-seeding and capture helpers live in replay.rs. The brief named traffic.rs; the car proof below went there.
- **Added `tests/traffic.rs` `the_cars_drive_on_from_a_rides_state`.** It seeds the 21 cars from four ride images (s146-roof, s129-across, s129-braked, s89-along). It then steps them with the port and with the original's `path_car_update` on the same RAM for 90 ticks, and compares every car's world matrix and cursor bit for bit. It also has a control for the old seeding rule.
- **Fixed S129's car seeding** in replay.rs `SaveState::traffic`. See "What the original does".
- Added a pointer to S146 in the doc comment of S136's stock-bike test.

## What the original does

- **A braked 250 on the moving train rides it and does not slip off.** Over 270 ticks (9 s) the bike's world x speed stays at 54.91–55.10 ft/s, with the wagon at about 55.15. It creeps back along the deck about 1.5 ft, roughly 0.17 ft/s, and sideways at most 0.6 ft. The capture's wreck_reason is 0 on every row. It never wrecks and never leaves the roof. S136's measured "8 ft/s back, off the deck at tick 90" does not happen with the real bike. That figure came from S136's stock bike and its hand-made controls; the platform carrying itself is fine.
- **Air drag reads world speed on the train too.** The 250's drag is 0.025. At 55 ft/s over the world it pulls about 38 units back, and the braked tires on the wagon hold against it. With the port's drag zeroed, the replay parts at tick 1. So the original's bits carry a drag that sees 55 ft/s, not the near-zero speed relative to the wagon.
- **Order of updates.** S129 had already settled this: path cars run before the bike in a frame. Here it holds on the bike's own lanes as well. The wagon's velocity changes in its low bits every tick (55.15688, 55.149555, 55.134907, …). The port's wheels read this frame's car motion (`Moving`, made by `Traffic::frame` before the rider steps), and the result agrees bit for bit for 270 ticks.
- **What the curve's "tangents dirty" word (car +0x4fc, research/oracle.txt S128) really means.**
  - 0 means the curve rides its cached tangents (+0x510/+0x520).
  - On a car that has moved, those cached tangents are its current segment's, which is the port's `stale: false`.
  - The port's `stale: true` means "the load's measured tangents".
  - S129's replay seeded `stale` from the word being 0. On every image that marks every car stale. In S146's state that swung the wagon about 60° off its rails on tick 1 and threw the bike, which parted at tick 1.
  - The rule now: a car is stale only while its cached tangents equal `Spec::measured` bit for bit. That is false for all 21 cars in all four images.
- **Some capture rows were read after the bike.** In five rows (161, 162, 179, 198, 211) the bike is a tick ahead of its frame. The row after each run repeats the bike, but its train has moved on.
  - Row 161 is the port's tick 162 on every lane; rows 162 and 163 are both tick 163.
  - Within S146's ride, `bike_ticks` numbers rows by the bike's own step (Δ`ch_cofg.x` over the previous row's `ch_vel.x`·dt, rounded) and asserts that each row's tick is within one of its frame's.
  - The train lane keeps the frame numbering, one tick on, as S129 does.

## How it was proved

- **Save-state capture replayed bit-exact** (`tests/replay.rs` S146): all 270 rows agree on every lane:
  - chassis velocity, quaternion, world and local cofg, angular velocities, histories;
  - both wheels' load points, normals, contact velocities, compression, spring force, spin, local velocity and force, ray_hit and swept contact;
  - the engine;
  - the slip flags;
  - the train clock.

  The test asserts the whole fixture agrees (`whole == None`), and that the original's bike stays within ±3 ft along the deck and never wrecks. The deck places match exactly: along the deck the original ranged −1.54..0.15 ft, and so did the port.
- **Drag control:** the same fixture with `mv.drag = 0` parts at tick 1, and the test asserts that it parts.
- **Car seeding oracle** (`tests/traffic.rs the_cars_drive_on_from_a_rides_state`): 4 images × 21 cars × 90 ticks of the original's own update on the image's RAM, every world matrix and cursor bit-exact. A control with the dirty-word rule puts cars off their place one tick on, in every image.
- S129's tests still pass with the fixed seeding. `the_bike_meets_the_dallas_train_as_the_original_does` keeps the same partings: braked at tick 2 (its known creep) and across at tick 148, so the old seeding was not the cause of S190.
- `./kit check -- -p game-sim --test traffic`: 11 passed. The three S129/S146 replay tests: passed.

## Not verified / not 1:1

- **The state is teleported, not ridden onto the roof.** The bike was moved onto a moving wagon and given its velocity by hand, then ridden 30 frames before the save. The capture starts after that settling, so the comparison is a parked bike already carried. How a bike lands on or jumps onto a wagon is not covered here; S191 asks for a recorded roof ride and a wreck.
- **One ride, one bike, one wagon, on the straight.** Only race250p on the second boxcar, braked, on the z≈2319 straight. From about row 245 the bike's z velocity grows (−0.04 → 1.5 ft/s by row 265), so the wagon may be starting into the curve in the last 25 rows. I did not check how much the wagon turns there. A turning wagon, which exercises the spin term of `Moving::at`, is at most touched at the very end; S136's oracle covers it function by function.
- **S136's stock-bike slip is not explained in detail.** I did not dig into why S136's stock bike (`Sim::new` with hand-set `ride::Controls`) slips 8 ft/s where the 250 does not, whether that is its drag, its brakes or the hand-held controls. It is a test-only bike; the original's 250 and the port agree.
- **`bike_ticks` relies on this ride's steady motion.** The rows read after the bike are found from the bike's step, which works only because it moves a near-constant 1.83 ft a tick (median step). `quarter_ticks` itself is unchanged and still misnumbers such rows in other captures. Filed as S192. S129's braked and across may have rows like these.
- **The update order is shown, not isolated.** That the bike reads this frame's car motion follows from the bit-exact agreement, given that the car's velocity changes in its low bits every tick. I did not run a control with the port stepping the bike before the cars.
- The capture, the image and the save states (copy 2 slot 13) exist only in s2's context. Elsewhere the S146 replay test and the image half of the traffic test skip and still read as passing.

## Next

S192 (a general rule for rows read after the bike). S190 and S191 are unchanged by this task.
