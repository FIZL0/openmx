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

## Why the drawn ride's rider still differs: the riding skeleton (R58)

Test: `r58_the_drawn_ride_s_thrown_rider_beside_the_original_s` (tests/replay.rs). It rides R57's
drawn ride (`r58_port_ride`) with three riding skeletons (`R58Skeleton`) and prints each beside the
capture:

- **Kit**: no `Rider::skeleton`, the kit's fixed riding pose (R57's ride).
- **Image**: the original's skeleton off recording2's image (`r24_posed`), held as
  `s474_ride_drawn` holds it.
- **Animated**: the port's own body, posed every tick as the shell does (`Body::ride` with the
  gas held, then `Body::ragdoll_posed`).

### The answer: the riding skeleton

It decides where the main bone rides, and that decides whether the bike box catches it.

- The riding main bone is kept in the bike box (`ragdoll_riding::BIKE_BOX`: z from -3.25 to +0.25
  ft in the bike's frame).
- On the strike tick the bike stops dead. The main bone travels on 1.77 ft (53 ft/s for one tick).
- Where the main bone rides on strike-1, behind the bike's origin, and what happens:

  | skeleton | behind the bike | main bone vz on the strike | truck wakes after the strike |
  |---|---|---|---|
  | original's (capture) | 1.81 ft | 29.9 | +10, +12 |
  | image | 1.71 | 29.4 | +10, +12 |
  | kit pose | 0.93 | 1.4 | +10, +12, +14, +19, +21 |
  | animated | 1.23 | 1.3 | +10, +12, +14 |

- A main bone riding less than ~1.5 ft behind the bike's origin reaches the box's front in that
  tick. The box catches it and sets it to the box's carried velocity: 0.8 × the stopped bike's
  1.8 ft/s = 1.44. That is R57's 1.4 ft/s.
- The rider then folds onto a still pelvis. The head lies on the face to +14, and the truck wakes
  on +14 too.
- On the image skeleton the box misses it. The hand-over speeds match (main bone 29.4 / 29.9, the
  others within ~3 ft/s), and the truck wakes on +10 and +12 only, as theirs. No bone gets behind
  the face.

### The other two suspects

- **The velocities at the hand-over**: not a cause of their own. They follow the pose: the image
  skeleton fixes them without other changes.
- **The bones against the hull**: not shown wrong. On the image skeleton the hull contacts from +3
  to +12 match theirs bone for bone (only +3/+4 differ by one bone).
- **The wreck's joint rests**: barely matter. Taking the skeleton away once the rider is down
  (the joints pull to the kit's rests) moves the legs on +20 by < 0.01 ft.

### Not matched yet, on the image skeleton

- **The head meets the hull a frame late** (+2..+13, theirs +1..+12).
- **The legs fall back slowly**: on +20 they are 1.9-2.5 ft out, theirs 4.2-4.9.
  - The original's feet (14, 17) meet the bumper on +2 and bounce back at 4 ft/s on +3.
  - The port's meet it on +3 and stay. They then slide back at 2-3.5 ft/s, theirs 5-8.
  - The main bone matches to ~0.15 ft through +20 (2.95 / 3.07 off the face).
- **Likely cause (not proven): the held image pose is not the pose at the strike.** It is
  recording2's pose, not the pose of a rider at full gas. Proving it needs the original's skeleton
  on each row (follow-up).
  - On strike-1 the original's pelvis rides 0.29 ft higher.
  - Its head, hands and knees ride ~0.3 ft further forward (off the main bone, z: head 1.02 / 0.72,
    hands 1.75 / 1.43, knees 0.80 / 0.50).
  - Raising the image skeleton 0.29 ft moves the legs to 2.5-2.9 ft out on +20. Not enough: the lean
    is the rest.
- **The port's animated body leans the other way.** Its main bone is 0.5 ft further forward, and
  its hands and knees ~0.4 ft further back. So in the shell the box catches the main bone, as on the
  kit pose.
  - R59: wrong. The test's body had no fade rates and held its bind pose. With the shell's rates it
    rides the original's pose (see R59 below).

### What this means for the S485/S486c tests

- S486c's wreck bike and truck push hold on all three skeletons: the rider does not move them
  (asserted in the R58 test).
- **They should ride with the bike drawn and the image skeleton.**
  - That ride wakes the truck on +10 and +12 only, through a head lying on the face, as the
    original's does.
  - The undrawn ride gets the same wakes through a wedged rider.
  - The drawn ride with the kit pose adds +14.
- Filed as T10. The animated body's pose (and a per-row capture of the original's skeleton) is R59.

## The animated body's pose was the test's, not the shell's (R59)

Tests: `r59_the_port_s_animated_body_rides_the_original_s_pose_at_full_gas` and
`r59_recording2_s_image_pose_is_a_stopped_rider_s` (tests/replay.rs). R58's test now rides the
animated body with the shell's rates.

### Why R58's animated body rode 1.23 ft behind

- **R58's ride never gave the body its fade rates.** `Rider::new` leaves `body_rates` at zero; the
  shell fills them (`rider.body_rates = vehicle_params()` in main.rs, for the human and the race's
  AI riders alike) and clears the start line.
- With zero rates every blend keeps ramp 1, which means all snapshot. So the body held its first
  pose (the bind pose) for the whole ride, though the machine asked for turn_riding_Mid.
- Not the clip, not the blend, not the rig.
- In the shell the body is posed right (`r59_as_the_shell` gives the test the same two settings).

### With the shell's rates

- At full gas the body plays turn_riding_Mid (state 30, two-clip blend with turn_riding_Fwd at
  mix 1, ramp 0).
- On strike-1 its nine riding bodies stand where the original's do, in the bike's frame, to
  0.064 ft (the test holds 0.1). Their velocities match to ~0.5 ft/s.
- On the drawn ride (R58's test):

  | | port, animated | original's |
  |---|---|---|
  | main bone behind the bike, strike-1 | 1.79 ft | 1.81 |
  | main bone vz on the strike | 28.8 | 29.9 |
  | bones on the hull, +2..+4 | the same, bone for bone | |
  | head on the hull | +2..+11 | +1..+12 |
  | truck wakes again | +10 | +10, +12 |
  | legs off the face on +20 | 3.5-4.35 ft | 4.2-4.7 |

- So the legs now fall back about as fast as theirs. On the image skeleton they end 1.9-2.5 ft out.

### The held image pose is a stopped rider's

- recording2's joint records (what `r24_posed` reads) are turn_stop_Mid at 0.5 s: rotations to a
  score of 0.0006 (the next clip scores 0.25), and the root's place to 0.004 ft.
- So R58's image skeleton is the pose of a rider on a stopped bike. Its main bone happens to ride
  1.71 ft behind (near the full-gas 1.81), which is why the bike box missed it.
- The rest of it is wrong for full gas, which is why the legs fell back slowly on that ride. The
  per-row capture of the original's skeleton is not needed for this: the original's bodies on
  strike-1 match the port's animated ones.

### Not matched yet: the strike's downward kick

- On the strike tick the original's bodies take a downward kick: vy main bone -3.5, neck -2.5,
  head -2.4, feet -7.4, knees -6.5. The port's: -0.9, +0.9, +1.5, -1.4, -1.5.
- It is not the skeleton: holding it from strike-3 changes the kick by < 0.01 ft/s.
- After it the port's body rises (main bone 0.3-0.9 ft above theirs from +2). Its head rides up
  over the hood, up to 0.42 ft past the face's line on +4..+8 without touching. The neck lies on
  the face instead (+5..+9), and the truck's +12 wake is lost.
- The head's first touch is a frame late on every skeleton (+2, theirs +1).
- Filed as a follow-up.

## The strike's downward kick is none of the bike's inputs (R60)

Test: `r60_the_strike_s_downward_kick` (tests/replay.rs). On R58's drawn ride with the animated
body, the strike tick's riding step is run again from strike-1's body, skeleton and holds
(`r60_strike_steps`), with one thing changed each time. The run again matches the tick to 0.003 ft/s.

### Row 58 is the feet switch, not the wreck's hand-over

- On the strike the bike is at 2 ft/s, under the stand speed (7.3). The feet switch
  (`Actor::feet`) lets both feet off their holds and turns their collisions on.
- The port does this on the strike tick, the same tick as the original. Row 58's on words are
  exactly that: the two feet's collisions (bones 14, 17) on, the other seven off.
- R57 called these "a read across the wreck entry". Wrong: the wreck enters on row 59. So the
  wreck's hand-over is not what row 58 shows, and is not the kick.
- The feet's touched word is cleared before the step, so row 58's zeros are real: nothing pushed
  the feet.

### What was tried on the strike step

vy after the step (main, neck, head, knee, foot, hand = bones 0, 2, 3, 13, 14, 6), and the main
bone's vz (over 20 means the bike box let it go, as theirs):

| change | 0 | 2 | 3 | 13 | 14 | 6 | vz |
|---|---|---|---|---|---|---|---|
| none (the port) | -0.86 | 0.92 | 1.52 | -1.53 | -1.37 | 1.35 | 28.8 |
| bike nose up 0.05 / 0.1 rad | -1.07 / -3.70 | 0.92 / 1.20 | 1.46 / -0.14 | -1.35 / -1.16 | -1.18 / -1.27 | 1.34 / 1.21 | 28.5 / 1.7 |
| bike nose down 0.1 rad | -0.47 | 0.93 | 1.62 | -1.85 | -1.72 | 1.36 | 29.3 |
| bike vy -3 | -2.52 | -0.93 | -0.26 | -3.34 | -3.21 | -1.04 | 28.8 |
| skeleton's legs 0.3 ft lower | -1.36 | 0.89 | 1.41 | -2.40 | -2.29 | 1.34 | 28.8 |
| gravity x6 | -6.23 | -4.44 | -3.85 | -6.89 | -6.74 | -0.52 | 28.8 |
| **the original's** | **-3.50** | **-2.46** | **-2.39** | **-6.48** | **-7.39** | **-0.35** | **29.9** |

- **Not it:** the bike's pitch (±0.2 rad), its velocity (vy ±3, vz 0 or 54), its pitch rate
  (±3 rad/s), its contact speed, and the skeleton's feet moved 0.3-0.6 ft. Also not a step 1.5 or
  2 ticks long, or two steps: those all let the bike box catch the main bone.
- Pitching the nose up 0.1 rad pulls the main bone down, but the box then catches it (vz 1.7).
- The pose damper and spring scaled (x0.5 to x4) move the torso up, not down.
- **What it looks like:** a hard pull down for that one tick, on the legs most. Six times the
  gravity puts the knees, feet and hands within 0.7 ft/s of theirs. It puts the torso 1.5-2.7 ft/s
  too far down.
- The port's actor has nothing like it. No request field is a force; the gravity scale is 1 and
  nothing writes it.

### What is left

Either the original's riding step on the strike reads something these changes do not cover, or
row 58 is read part-way through a later write (PINE reads while the game runs). To tell which, it
needs a capture on the original. Filed as a follow-up:

- Record S484's ride twice, to the bit, so a torn row shows as a difference between the two runs.
- Add on each row:
  - the bike's transform (the rows next to veh+0x1b0);
  - the body actor's request (velocity, rates, speed, the gravity scale);
  - the bones' orientations.
