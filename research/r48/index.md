# R48: the biplane taxied into free09's parked truck, on the original

C16's 2p session parks no truck (both machines are planes; no car vtable 0x0045edac in its images),
so R48 booted its own free09 for one player with the plane and the truck parked.

## Steps (kit copy 2, accessibility bus on)

1. `bash research/r48/start.sh` cold-boots copy 2 and runs `boot.py`: the S149a boot with event
   +0x11c = 1 (`[Biplane1]`), +0x128 = 4 (`[Monster2]`), track free09, one player. Slot 1.
2. Slot 2: the switch cheat set. `context/recordings/r48/nav.p2m2` (from slot 2) opens the pause
   menu and switches to the plane. Slot 3: in the plane.
3. `aim.py PID 3 0 -45 5`: the plane put back 45 ft south of the truck's body, nose at it
   (`+z`), dropped onto its wheels. Slot 5, frame 1487. East tipped it on the hill; west faces the
   barn; north has posts close.
4. `context/recordings/r48/drive.p2m2` (from slot 5): Cross 160 fields (80 ticks), idle 440.
5. `KIT_PCSX2=2 ./kit pcsx2 with -- bash research/r48/play_capture.sh PID TAPE 1488 290 OUT`
   with `watch/r48-truck.toml`; `show.py OUT` prints it.

## Files (shared context)

- `capture/r48/truck.csv`: rows 1488..1777, no gaps, dt 0.03333 every row.
- `states/r48/r48-1487.ram`: slot 5's EE RAM (the drive tape's state).
- `recordings/r48/{nav,drive}.p2m2` and their `_SaveState.p2s`.

Addresses in this boot: plane 0x007ccae0, truck car 0x01490240 (body at +0x160), the human's bike
0x015613d0.

## What it shows

- The throttle (+0x34c) ramps 0.0117 a tick and holds at 0.933 once Cross ends.
- Row 1569: the truck wakes, velocity (0, -0.39, 14.02); the plane 32.38 -> 21.86 ft/s, contacts 0 -> 1.
- The plane keeps its throttle on the truck and pushes it ~40 ft (z 3877.7 by row 1628). The truck
  stays awake to the capture's end.

## The Sim against it (crates/game-sim/tests/c16_fly.rs, `r48_*`)

- Truck settles within 0.11 ft of the original's body.
- Taxi rows 1488..1568: throttle exact, plane within 0.02 ft and 0.1 ft/s.
  The first tick after the load reads the idle pad.
- Row 1569: wakes on the same row, shove 14.12 vs 14.02, plane 21.859 vs 21.864.
- Before S487 the Sim's plane stopped dead about 13 ticks on, and the truck moved ~5 ft and slept
  from about row 1595.
- S487: the woken truck now steps after the plane, as the original's bike world does.
  - Its record takes the plane's mirrors. A spring only pushes its own side, so without them the
    plane's rays never pushed the truck after the first impulse.
  - Its own rays strike the plane's hull. This doubles the push, and is what holds the truck.
  - Rows 1570-1584 now match along z: place within 0.3 ft, speed within 2.6 ft/s. That covers
    1571-1573 with no plane contact, the 18.9 ft/s kick at 1574 and the climb to 25 ft/s.
  - Both stay awake to 1640. The truck reaches z 3872.5 by 1628 (original 3877.2).
- Still off from ~1585: the Sim's plane drags at ~21 ft/s against the original's 24-30. So the
  truck runs ~7 ft/s slow, ends ~5 ft short and still rolls at 1640.
- The wreck-bike test is not captured: this boot has no P2.

## The original replayed (S489)

- Test `r48_the_originals_node_update_replays_the_push`: Ee off `states/r48/r48-1487.ram`, gp =
  boot_gp. Each tick: write the row's `cmd_34c` to plane+0x34c, then
  `ee.call(0x26f800, [0x7d0600], [row dt])`.
  - Node 0x7d0600: count at +0x30, objects at +0x38. The plane's object (plane+0x170 = 0x144a030),
    then the truck's (truck+0x170 = 0x14696d0), then 0x8eb230.
  - It gives the capture on every row 1489..1640 within 0.001. Row 1488 is 0.002 off (saved controls).
  - Row 1569 is torn. The capture's 21.86 / 14.02 is what the first substep's impulse left. The tick
    ends at plane 21.62, truck 14.54 (x -0.41). So the existing meeting test compares the Sim with
    the torn row.
- Resolve hook 0x270dc0 (a0 = object, a1 = first). Contact list: count at obj+0x100, array at
  obj+0x108, 0x80-byte entries, kind at +0, other object at +0x74.
  - Each substep (4 a tick): plane resolve, then truck resolve. The truck resolves on its wake tick.
  - Plane list: its own entries (kind 0), then mirrors (kind 1) of the truck's strikes.
  - Truck list: the plane's mirrors first (kind 1), its own entries on the plane's hull after
    (kind 0, from 1582).
  - 1569-1574: only the plane's first-substep resolve changes speeds (the impulse). The truck's
    resolve of its mirror changes nothing.
- The Sim does it differently. The truck steps after the plane's whole frame, files the mirrors after
  its own entries, and its strikes' mirrors never reach the plane's list.

## The truck in the plane's world (S489, landed in S489a)

- `s489-truck-in-plane-world.patch` (applies to 33a1f54e7): the woken truck searches after the plane
  and steps as a body of the plane's world, as the oracle orders it.
  - Substep logs then show the oracle's pattern: one impulse, then the truck's resolve changes nothing.
  - But `r48_the_plane_keeps_pushing_the_truck` fails from row 1576 (truck 17.1 vs 20.6 ft/s).
    S487's order passed to 1584 because the truck's end-of-frame resolve against the stepped plane
    gave pushes the original doesn't have.
- With the patch and the truck put at the image's body (not the settled spot, which is 0.10 ft
  further along z), row 1569's impulse matches: plane 21.879 after it (oracle 21.864), truck 13.990
  (14.023).
- What is left is the truck's own forces between impulses (its tires):
  - Wake tick per substep, truck vz: oracle 14.023, 13.404, 13.758, 14.354, end 14.535.
    Sim 13.990, 13.382, 13.750, 14.175, end 14.242.
  - From the third substep the Sim's truck gains less. By 1573 it runs 1.4 ft/s slow. The plane then
    strikes it harder and drags.
  - Likely the woken truck's tire/suspension state: the Sim's comes from `Parked::settled`, the
    original's from its parked state in the image.

## The struck truck's brakes (S489a)

- The patch landed (car::InPlane, Met::cars). The file `s489-truck-in-plane-world.patch` is deleted.
- The R48 tests now put the truck where the tape's state holds it (`r48_seat_truck`: body, tires,
  machine words and engine words off the image), after the settle check.
- What the woken truck lacked: drive torque on the rear tires.
  - Oracle wake tick 1569: `car_tire_add_drive` (0x39b660) gives rear tire 2 a torque of 92.74 on
    substep 1. That is output_torque 9.048 × ratio 25 × (1 − 0.18) / 2. The Sim gave 0.
  - The Sim zeroes drive while braked, and its undriven truck had rear_brake 1.
  - The original's brakes (car +0x3f4/+0x3f8) were 0. A store log (`ee.log`) over each node update shows
    why. On a tick the plane strikes the truck:
    1. the enable 0x398c50 writes driven (+0x4ac) = 1 (pc 0x398c54),
    2. the controls step's driven path writes the brakes 0 (pc 0x396aac / 0x396ab4),
    3. only then the car step clears driven (pc 0x3e1934, when the body is awake).
  - On a tick it is not struck (1571-1573) the undriven path writes them 1 (pc 0x396bbc / 0x396bc0).
  - So "the first step clears +0x4ac before its controls read it" (oracle.txt, S484) is wrong:
    the clear comes after the controls.
- Fix: `InPlane::struck` sets the driven word with the wake; `InPlane::preforce` clears it after the
  controls step. The riders' world (`Parked::struck`, S485) is not changed.
- Now: wake tick end plane 21.624 / truck 14.537 (oracle 21.624 / 14.535). Rows 1570-1579 within
  0.01 ft. `R48_PUSH_THROUGH` is 1587 (was 1584).
- Left: from 1573 the braked rear tires stick-slip (force z flips ±4000 each substep). Small
  differences (rear spin after 1570: Sim 0.024 / -6.486, oracle 0.088 / -6.382) grow. The truck is
  3 ft/s slow at 1588, and from 1615 the two part (the Sim's truck climbs, vy 6-15).

### In the riders' world (S491)

- Tried: `Parked::struck` set the driven word, `Parked::frame` cleared it after the step (as
  `InPlane`). The car steps the frame after a bike's strike, so it drove brakes-off then.
- S484's `ride1.csv` says no. Strike+1 vz: brakes-off 2.533, ride1.csv 1.752, undriven (as now)
  1.744. Brakes-off, the leaning wreck bike strikes it every frame and it drives on at ~1.7 ft/s
  through strike+15 (ride1.csv: asleep from +13, slid 0.203 ft). Both S485 tests fail with it.
- So the original's truck is braked on the step after a riders' strike. The change was reverted:
  `Parked` stays undriven.
- Not known: why. The clear (pc 0x3e1934) is in the truck's post 0x003e1710, after the controls,
  so either something else clears +0x4ac between the riders' node and the car's next step, or the
  bike's wake does not go through the enable 0x398c50. A store watch on car +0x4ac over S484's ride
  (research/s484/truck.sh) would tell.
