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
  - Replaced by S489 (below): the stick-slip came from the brakes the Sim left on; the push now
    holds through 1604.

## What slowed the plane on the truck (S489)

Per substep, the oracle's truck (hooked after its tire step 0x397f40) beside the Sim's, rows 1569-1600.

- **The launch boost.** The image's engine has launch boost (engine +0x6c) 1.886, launch timer
  (car +0x42c) and start (+0x430) mid-launch; the settled truck had boost 1. The boost scales the
  drive torque while the tire spins forward, so the Sim's truck gained less off the wake (1.4 ft/s
  slow by 1573). `r48_seat_truck` now takes the three words off the image. Rows 1569-1579 then
  match within 0.008 ft/s.
- **The truck's own strikes on the plane.** From row 1580 the Sim's truck braked on some ticks
  (1580, 1588, 1589) and the original's never did after 1574. A hook on the enable 0x398c50 shows
  two callers:
  1. ra 0x15ad6c: the plane's rays striking the truck (S489a's `InPlane::struck`).
  2. ra 0x15ad28: the hit listener 0x15acf0 (S200), from row 1580, alone on 1588-1589. It is the
     truck's own rays striking the plane's hull: wrapper +0x58 set and the struck collidable's
     +0x84 & [0x468f68] (0x40), it calls vt+0x124, the enable.
  - Fix: in the plane's frame (`Flight`, the parked cars), a car whose own search struck the hull
    calls `InPlane::struck` too, before its controls step.
  - S507: a driven car's world (S499, `Machine::cars_met`) does the same for a parked car's rays on
    the driven car's hull. Not yet checked against a drivers' capture (S499 has none).
  - Now: rows 1569-1587 within 0.008 ft/s on the first substep, brakes 0 on both. The push holds
    to the capture through row 1604 (`R48_PUSH_THROUGH`, was 1587); row 1605 fails (truck 1.7 ft/s slow).
- **Where it parts: the plane's wing ray at row 1587.** The ray from (0, 4.47, -4.66) to
  (7.5, 2.40, -1.63) in the plane's frame meets the truck's hull facet 31 at t 0.974 (1585) and
  0.987 (1586) in the Sim. At 1587 the original still meets it, 0.0015 ft short of the ray's tip
  (about t 0.9998); the Sim's ray does not reach it (its box test drops the facet). The Sim's truck
  is 0.0015 ft ahead there, from the 0.002-0.008 ft/s left since row 1572. The lost contact gives
  the Sim's plane a sideways kick on 1587's second substep, and the two part from 1588.
  - S508 settled it. See "The last difference (S508)" below.

### The last difference (S508)

Per substep, the Sim's truck beside the original's RAM at its engine step (hook on 0x399e40), in
the node update 0x26f800, replayed off `states/r48/r48-1487.ram`. Every field was diffed. The Sim
matched the capture through row 1568. The first difference is inside row 1569's first substep, not
at 1572.

- **Cause 1: the tire mounts.** Each tire's mount (tire +0x80; tires at car +0x548 + 4k) differed
  before any force. The lever x was -6.19055 in the Sim and -6.1901364 in the original. The other
  tire was +6.1905794 and +6.191113. The y differed by 1.7e-6 and z by up to 2e-4.
  - The original's mounts come off the placed model, so they carry the round trip's rounding.
  - The Sim's come off the unplaced model in `Machine::new`. Its doc already says so.
  - The test's `seat_truck` did not take them off the image.
- **Cause 2: the inertia.** The body's inverse inertia (+0x120) and the collision object's copy
  (car +0x170 object, +0x80) differed in the last digits. The copy: Sim (9.880182e-5, 9.034243e-5,
  1.3870311e-4). Original: (9.880171e-5, 9.034181e-5, 1.3870181e-4). Taken to be the same rounding (not traced). It changes the
  wake impulse's last digits.
- **With only the mounts and the body's inverse inertia seated,** the truck and plane stay within 0.0007 ft/s through row 1610. The
  1587 wing ray is met. With the collision object's copy too, they are bit-identical.
- **The fix is in the test.** `seat_truck` in `crates/game-sim/tests/c16_fly.rs` now takes the
  mounts and both inertias off the image. `tests/car_spawn.rs` already does this for mounts and body
  inertia. No shipped code changed.
- **Result.** `r48_the_plane_keeps_pushing_the_truck` now asserts position and velocity, all axes,
  of the plane and the truck. They are bit-identical to the capture rows 1570 to 1640, the tape's
  last compared row. `R48_PUSH_THROUGH` is now 1640, was 1604. Before, z held only within 0.3 ft and
  2.6 ft/s.
- **Checks.** The truck steps the same on every substep as the original's RAM, for ticks 1569 and
  1570. All 19 `c16_fly` tests pass. R53's tests pass with their results unchanged.

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

## The thrown rider against the nose (R55)

S486c's lined-up ride (`S486C_BIKE`, tests/replay.rs), probed with temporary prints (all removed).
The strike is port tick 59; the rider is thrown at 54 ft/s (reason 1, throw (0, 400, -191) on the
main bone for 0.2 s, hands held to +9). The nose's face is the hull's -z face at z 3831.2; the
hull's underside is at y 135.95, so every bone (y 136.4-140) meets the face, none goes under it.

- **Why the bones get through: they tunnel, they are not pushed through.**
  1. On strike+1 (the first wreck frame) the bones' search is not built (no sphere has stood while
     riding, S105), so no bone meets the truck that frame.
  2. The ragdoll world steps one substep a frame (rate 25, dt 1/30) and integrates before its
     three constraint passes. At 54 ft/s a bone moves 1.8 ft before anything tests it.
  3. The sphere-mesh test is one-sided: a centre past a face's plane gets no contact from it.
  4. The hull's class (0x52058) is not in the bone sweep's include list (0x20, S166), so nothing
     sweeps it back out.
  - Bone 17 is behind the face on +1, bones 3 and 16 on +2, bones 0 and 2 by +4: five bones end
    inside the hull for the whole ride (four after +20). The joints hold the rest against the face,
    so a front sphere touches it every frame and wakes the truck every other frame.
  - Not deep first contact and not the joints: nothing has met the face when they cross.
- **Lending the hull on the first frame too** (temporary: the parked cars listed around the first
  sphere's centre while the search is unbuilt): no bone stays behind the face after +2. The wakes
  at +10 and +12 still match ride1.csv's. But the head (bone 3) leans on the face, and the truck
  wakes every other frame through +18 (the original's sleeps from +13). From S485's spot the
  put-back does not move (port tick 138, the original's row 147).
  - So the wedge is not the whole story, and it is not why the put-back is 9 frames early.
- **Is the wedge the original's?** Probably: the first frame's empty search is S105's model, and
  S166 says the original's bones tunnel through moving collidables too. Nothing in the port
  should change on this evidence.
- **Should the port's bones leave the nose by +13?** ride1.csv cannot say: it has no bone rows.
  Its truck sleeping from +13 says only that the original's bones stop striking the hull by then.
  Wedged or not, the port's keep striking it, so something else differs (the throw, or how the
  rider falls off the face).
  - What would tell: S484's ride re-recorded with the rider's ragdoll part bodies per row
    (research/s484/ride.py), compared with the port's bones from strike+1.
- **Answered by R57** (research/s484/index.md): the original's bones never get inside the hull; they
  come in at 24-30 ft/s from the riding bodies, and the head leaves the face on +13. The wedge is the
  test ride's: with no drawn bike it has no riding ragdoll, so its bones enter at the bike's 54 ft/s.
  With the bike drawn as the shell draws it, no port bone gets behind the face either.
