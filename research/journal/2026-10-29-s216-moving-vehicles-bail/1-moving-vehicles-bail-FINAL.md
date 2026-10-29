# 1 — moving vehicles bail (FINAL)

Task S216. 2026-10-29.

## What I did

- **Rode free06's vehicles in the port's shell** (headless, `--trace-at`, generated pad tapes), with a
  temporary per-tick print of the bike's g loads, its speed and the nearest car (now removed):
  - **The train:** S137's and S170's four rides (side at full gas, nose head-on, nose from behind, a
    bike inside a car).
  - **The construction trucks:** the dump trucks (13–16), the scrapers (17–20) and the ctrucks
    (21–22), from both sides at the trucks' places at t = 6 and 8 s. The bike rode in on full gas, or
    on a 25-frame burst and then coasting. It started 30 ft out, so it arrived at 20–36 ft/s.
  - **Rear hits:** a standing bike that ctruck 22 runs into from behind, and a bike rear-ending
    ctruck 22.
- **Found the check that throws the rider at once:** it is not the wreck test's g limits. It is
  `mover_contact::inside`, the port's own fallback (S137). When the head ray's end is inside a car's
  root box, it adds a scenery contact, and the wreck test reads that as reason 6. A train car fills
  its box, but free06's dump truck's box is 28 ft wide, 26 tall and 46 long, round a body that
  narrows under its bed and between its wheels.
  - Riding up beside dump truck 13 at a glancing angle (`2955,3800,240`, a 25-frame burst) came off
    with reason 6 on the first tick the head entered the box. At that moment g was 0 and nothing was
    struck: the bike was 13–14.5 ft from the truck's centre line and 0.9 ft or more from any face.
  - With the fallback off, the same ride rolls on beside the truck for 200 frames with g 0
    throughout.
- **Changed the fallback** (`crates/game-sim/src/mover_contact.rs`, `inside` and the new
  `enclosed`). The head ray's end now counts only when one of two things holds:
  - **The car's mesh shuts it in.** A ray out of each side, each end and the roof meets one of the
    car's faces, of either facing. The train car's mesh needs both relaxations: its end faces point
    into the car, and it has faces inside. The floor is not asked, because the cars are open
    underneath.
  - **The car's leading end came over it this tick.** The box face nearest the end is the z end the
    car drives towards, and the end lies no deeper than the car's step (velocity × dt).
- **Why the second rule is there:** S170's from-behind ride needs it. The bike is carried for 7 ticks
  and then comes off with reason 6. The point where the port's head comes off is 0.29 ft inside the
  locomotive's box, but **outside its mesh**: the nose narrows inside its box, and no face lies on
  either side of that point or above it.
  - With only the mesh rule, that ride never wrecks.
  - With "nearest the leading end" but no step limit, dump truck 16 met in its flank 3.65 ft back from
    its front threw the rider with nothing near the head (q 9.9, 5.6, 18.6 in the truck's frame).
- **Tests:**
  - New `crates/game-sim/tests/moving_vehicles.rs`. The disc's own collision meshes are driven
    straight across a flat yard as the traffic hands them over (`Prop::stand` and `Moving::of` each
    tick, `traffic.first = Some(0)`), and the 125 settles braked for 60 ticks first. Three cases:
    - free06's dump truck crawls by at 6 ft/s beside a bike rolling at 5 ft/s, the bike's head in its
      box for 64 ticks. It rides on: struck 23 ticks and not wrecked. Against HEAD's
      `mover_contact.rs` it came off with reason 6 on tick 0.
    - A train wagon (`train_car_03`) at 54 ft/s is met side-on by a bike at 8 ft/s. The bike is pushed
      back (it ends at −0.95 ft/s along its heading) and is not wrecked.
    - free09's trailer (`free_trailera`) at 30 ft/s is met the same way: pushed back to −2.2 ft/s, not
      wrecked.
  - New `crates/openmx/tests/train.rs` `a_bike_riding_up_beside_the_dump_truck_rides_on`: the
    user's ride on Dallas in the shell (`2955,3800,240`, a 25-frame burst, 200 frames), no wreck.
    S137's and S170's four train tests are unchanged and still pass.
- From opentennis I took nothing: it has no vehicles.

## What the original does

- **The wagon's side:** S129's `across` capture is the original meeting a moving wagon's side: it
  is pushed back and rides off, with no wreck. The port's replay of it (`tests/replay.rs`,
  `the_bike_meets_the_dallas_train_as_the_original_does`) gives no wreck in either ride, as before
  this task.
- **The train's nose:** S170's captures are the original's nose meeting a standing bike.
  - From behind, the train carries the bike up to 0 → 19 → 28 → 31 → 42 → 45 → 49 ft/s over 7 frames,
    and then it comes off with reason 6. The port gives 14, 27, 34, 47, 50, 55, then 6.
  - Head-on, it gives reason 1 at once, in the original and in the port.
- **Its rays:** the original has no box test. Its body rays strike the faces they cross, and the
  wreck test (S156) reads per-frame shocks against the bike's limits. For the race 500, those are
  back 22, front 18.33 and side 19.07 ft/s. So a bike beside a truck whose head is clear of its body
  rides on.
- **The trucks:** no capture of the original hitting a construction truck or a free09 trailer
  exists. The S159/C12 truck rides are the 2p state's physics truck, not a path car. What the port
  gives there comes from those same rays and limits:
  - A bike riding into a truck's flank at about 30 ft/s goes over the bars (reason 1, g.z about −45).
  - A head that reaches into the truck's body is struck scenery (reason 6).
  - A standing bike rear-ended by ctruck 22 at 28 ft/s gets a 25.7 ft/s shock against the back limit
    of 22, and comes off with reason 4 on the first tick.

## How it was proved

- The shell rides above, read off the temporary per-tick print, before and after the change:
  - The glancing dump-truck ride: reason 6 on the first tick before, no wreck after.
  - S137's full-gas side: 1. S170 head-on: 1. S170 from behind: 6. Inside a car: [6]. All the same
    before and after.
- `./kit check -- -p game-sim --test moving_vehicles`: 3 passed. With HEAD's
  `mover_contact.rs`, the dump-truck test fails (reason 6 on tick 0), and the wagon and trailer
  tests pass both ways.
- `./kit check -- -p openmx --test train`: 5 passed.
- `./kit check --all` from `./kit done`.

## Not verified / not 1:1

- **No capture of the original hitting a construction truck or a free09 trailer.** The port's
  answers on the trucks are its rays and S156's limits. These include reason 1 into a flank at about
  30 ft/s, reason 6 for a head into the body, and reason 4 for a standing bike rear-ended at 28 ft/s.
  None of them is compared against the original. I kept them unchanged rather than guess: they are
  the same rays and limits that match the train's captures.
- **`enclosed` and the leading-end rule are the port's own.** They stand in for the original's
  chassis sweep, which is not ported. Both are tuned on these meshes only: the dump truck,
  `train_car_01` and `train_car_03`. Other limits:
  - Neither rule is checked against the scrapers', the ctrucks' or the trailers' meshes by a
    targeted ride. The yard tests and the sweep only show that riding into them does not throw the
    rider before contact.
  - A car whose mesh has a closed pocket around an open-air space would still count a head there as
    inside.
- **free09's trailers were met only in the game-sim yard** (one trailer, side-on, 8 ft/s), not
  ridden in the shell at their places on the track.
- **The yard rides are not the track.** The floor is flat, the cars drive straight at constant
  speed (no spin), and the bike is the 125 given its speed after a braked settle, with its wheels
  not spinning at that speed.
- **The port's pass-through of a wagon in S129's `across`** (S186's open gap) is unchanged. The bike
  is not wrecked, as in the original, but it goes on past the rail.
- **The sweep's slow rides were not slow.** The truck-side sweep started 30 ft out, so even the
  25-frame burst arrived at 20–30 ft/s. The only truly low-speed truck meetings are the glancing
  dump-truck ride and the yard tests.

## Next

- Capture the original riding into a construction truck's side and being rear-ended by one: filed as
  a follow-up.
