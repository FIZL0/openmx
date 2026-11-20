# S484: a bike ridden into free09's parked truck, on the original

`truck.sh` + `ride.py` (kit copy, accessibility bus on): recording2's bike moved 60 ft south of the
truck's nose (x 4594, z 3778), gas held, one row a game frame. The ride is deterministic: two runs
give the same truck and bike columns to the bit.

- `context/capture/s484/ride1.csv` (S484): the truck's body and the bike's place.
- `context/capture/s484/ride2_bones.csv` (R57, `truck.sh ... bones`): ride1.csv's columns (equal to
  the bit, 240 rows, no torn or skipped rows) plus the rider's nine bone collisions.

## The thrown rider against the nose (R57)

Run: `KIT_PCSX2=1 ./kit pcsx2 with -- bash research/s484/truck.sh PID 4594 3778 240
context/capture/s484/ride2_bones.csv bones`. Test: `r57_the_original_s_thrown_rider_leaves_the_truck_s_nose_on_strike_13`
(tests/replay.rs) holds the original's facts and prints the port's beside them.

Per collision (actor +0x624, the rider file's order: bones 14, 17, 0, 2, 3, 6, 10, 13, 16): the
bone body's awake, place, velocity, and the collision's touched (+0x14), scene (+0x1c) and on (+4).
"The face" below is the hull's -z face, 6.81 ft ahead of the truck's body (R55's 3831.2 against the
port's resting truck). The hull is not a box: bones meet it up to ~0.6 ft further out too.

### The original's (strike = row 58, the truck's first awake row)

- **No bone gets inside the hull.** No centre passes the face in rows 58-97. The nearest is the head
  (bone 3, radius 0.75), 0.18 ft short of the face on strike+5.
- **The bones come in slow.** On the strike row they move at 24-30 ft/s (hands 7), not the bike's 54:
  the riding bodies, held to the bike that has just stopped dead. On +1, 31-39 ft/s.
- **The wreck enters a tick late (B19).** The collisions are off (on = 0) through row 57; row 58
  reads the first two on (a read across the entry), row 59 all nine.
- **Who touches the hull, when:** the head on +1; bones 14, 17, 13, 16 on +2; 0 and 2 on +3..+5;
  from +6 only the head. The head lies on the face through +12 (the truck's at-rest wakes on +10 and
  +12) and leaves it on +13, when the truck sleeps for good. No scene touch after +12.
- **Then the body falls back off the nose.** From +7 the legs (14, 17, 13, 16) slide back ~0.24 ft a
  frame (4.2-4.9 ft out by +20); bones touch only the ground from +23.

### The port's, on S486c's lined-up ride (`S486C_BIKE`)

- **As the tests ride it (no drawn bike):** no riding ragdoll, so the bones enter on the strike at
  the bike's 54 ft/s. Five end behind the face (R55's tunnel), and the head touches to +21.
  - So R55's wedge is the test harness, not the game: the original's bones never tunnel, because
    they arrive at half the speed.
- **With the bike drawn as the shell draws it** (`r57_port_ride(true)`: the chassis posed after each
  tick and its holds handed to the next, as in `s474_ride_drawn`; the kit's fixed riding pose, no
  animated skeleton):
  - The bones enter a tick late from the riding bodies, at 19-22 ft/s on the strike and 29-34 on +1.
  - **No bone gets behind the face** (the test holds this).
  - Not matched:
    - The head first touches on +2, not +1. It lies on the face +2..+14 and leaves on +15, two
      frames after theirs.
    - The truck wakes on +14, +19 and +21 too. Theirs wakes only on +10 and +12.
    - The legs slide back off the nose slower: 1.4-2.4 ft out on +20, theirs 4.2-4.9.
    - The main bone (0) rides into the strike at 1.4 ft/s, theirs 29.9.
  - Filed as a follow-up.

### What this means for the S485/S486c tests

- `the_lined_up_wreck_bike_slides_back_off_the_truck_as_the_original` and the F73 wakes ride with no
  drawn bike. Their bones are not the game's.
- Their +10 and +12 wakes match ride1.csv's, but by a different path: a wedged rider, not a head
  lying on the face.
