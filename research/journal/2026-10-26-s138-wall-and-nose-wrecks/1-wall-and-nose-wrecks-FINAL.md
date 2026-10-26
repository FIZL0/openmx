# 1 — wall and nose wrecks (FINAL)

Task S138. 2026-10-26.

## What I did

- Recorded the original's bike (recording2's rider, slot 3) teleported about free09's farmyard
  (research/s76_tele.py) into the garage shed's walls and onto its front wheel, each with save states a
  few ticks before the hit: research/s138_hits.py (S67's whole-bike spec, research/s138/hits.toml, plus
  the wreck reason), research/s138/wall.sh, research/s138/nose.sh and research/s138_nose.py (a nose
  landing made by hand: an upward velocity, then a forward pitch rate once airborne), pad scripts in
  research/s138/*.pad. Captures and RAM images in context/capture/s138 (git-ignored).
- Found why the port never wrecked on a wall: the riding bike's collision object searched only other
  riders' frames; for world props the port used a velocity-bundle stand-in that only set "touching" and
  never loaded the chassis shock, so no g peak and no FrontWreckSpeedLimit. Ported the body-ray search
  against the world's props and trees: `rider_contact::scenery` (the eleven bikem.col rays through
  `flight_search::Scene::wooded` and `contact_search::collide` with the bike's record flags), merged
  into the frame's `Bump` (or a bump of its own, `rider_contact::alone`) in `rider.rs`, and
  `Ride::frame_searched` which skips the stand-in when the props were searched.
- Found why a 70° nose landing wrecked in the port and not in the original: the wreck test's pitch is
  the euler the bike's attachment walk measures *after* the physics (step 4b in `ride.rs`), not the
  top-of-frame one.
- Test: crates/game-sim/tests/replay.rs
  `walls_and_nose_landings_wreck_as_the_original_does` — eight captures replayed from their states in
  free09's world, the port's first wreck (reason and tick ±1) against the original's.

- Knock-on: the step 4b measure moves every ride, so platforms.rs's vectors (HASHES, TRACE_DIGEST,
  FINAL_DUMP_DIGEST, FIXTURE_TEXT_DIGEST, RIDERS_*) and ride.rs's MD25 folds were re-pinned (spliced
  from `print_the_vectors` / the assert's left side; POS_Y, VEL_Y and the dump's head did not move).
  V50's `the_shell_rides_into_a_free09_tree_and_stops_at_its_trunk` (crates/openmx/tests/trees.rs):
  the body rays now strike the trunk and, head on at 56 ft/s, wreck the bike on that tick (speed
  56 → 13.6, at 36 ft of 40), as the walls do; the test read the respawn 48 ft along as riding past
  it. It now ends the ride at the wreck and takes a wreck as a stop.
- Tried and dropped: keeping the old body probe on ticks the rays touched nothing (as a stand-in for
  the sweep back) stopped the 92/s wall a tick before the rays reached it, without a g peak, so no
  wreck: the old probe is not swept at all when the props were searched.

## What the original does

- The riding bike's collision record (bike +0x7d0: search ground 0, search 1, sweep back 1, capacity
  11, nearby) is searched against the world's props with the chassis' body rays, the same as against
  other riders. `./kit fn 0x002522c0` (bike_wreck_test) reads the chassis' g peak from the shock that
  the resolve's first-substep impulse adds into; a head-on wall gives g.z −32.9 against a front limit of
  18.33 → reason 1 on the next tick. A corner post clipped 2 ft in at 46/s gives a sideways kick (x accel
  79.6, yaw 0.26) under the limits and rides on.
- The riding branch of the bike's vehicle update calls the attachment walk (`./kit fn 0x00133500`) after
  the air state and before the stunt clocks; it calls each enabled child's vt+0x38, the chassis'
  being chassis_measure (`./kit fn 0x0015c4c0`), which re-takes the euler (+0x90) and speed (+0x34). So
  the wreck test's pitch is post-physics: a nose 70.06° down in the air measures 69.86° after the
  touchdown's correction and rides out under GroundPitchDownWreckAngle 1.2217 (70°); 74.95° → 75.03°
  wrecks reason 1 on the touchdown tick.
- Notes and captured numbers: research/oracle.txt, S138 section.

## How it was proved

A save-state capture per case, replayed in the port from the same state with the captured pad
(`PadTape::from_capture`), the power curve first checked against race125p, the impact tick's g,
velocity and pitch printed beside the original's (the wall's matched to print precision: g 0.97 /
18.24 / −32.88, vel 3.88 / 5.98 / 59.04, pitch 51.01°). Eight cases, all agree:

| capture | hit | original | port |
|---|---|---|---|
| wall | back wall head-on, 92/s | reason 1 @2856 | reason 1 @2855 |
| slow | back wall, 29/s | reason 1 @2772 | reason 1 @2771 |
| gprobe | drifting into the back wall's end, 50/s | reason 1 @2815 | reason 1 @2815 |
| rub2 | south-east post clipped, 46/s | rides on | rides on |
| nprobe3 | nose 62°, 30/s down | rides on | rides on |
| nprobe4 | nose 66°, 44/s down | rides on | rides on |
| nprobe6 | nose 70.06° → 69.86° | rides on | rides on |
| nprobe7 | nose 74.95° → 75.03° | reason 1 @2792 | reason 1 @2792 |

With the scenery search switched off (a temporary env switch, since removed) wall, slow and gprobe all
rode on in the port, so the test catches the bug; before the step 4b measure nprobe6 wrecked in the
port at 2795.

## Not verified / not 1:1

- The chassis sweep back (collision_record_sweep_back) against props is not ported. With the
  stand-in skipped when props were searched, a thin prop could be tunnelled at high speed; none of the
  eight cases does.
- Bike-against-bike and bike-against-prop contacts are appended into one list, not found in the
  original's walk order, and do not share one capacity of 11 / one contact a ray between them.
  Only cases with one kind at a time are captured.
- The node-distance (`near`) the stand-in fed gets nothing from the body when props were searched.
- The attachment walk's speed (+0x34) re-measure is not ported: only the euler. The trick request
  reads the top-of-frame speed; not checked against a capture.
- That the chassis is among the walked children is inferred from the captures (70.06 → 69.86 riding
  out, the post-physics pitch matching), not traced through the walk's list.
- Ticks agree within ±1, not exactly: capture rows are read at varying phases (memory note
  "capture rows read at three phases").
- The nose landings are hand-made (velocity and pitch-rate pokes into a standing bike), not a human's
  riding; the load at touchdown is what the throw gave, not a sweep over loads.
- The head ray's hits are unchanged.
- No capture of the original against a free09 tree trunk: that the trunk wrecks at 56 ft/s rests on
  the same limits as the captured walls, not on a tree capture.
- The re-pinned platform vectors are this machine's only; no cross-arch run.

## Next

Follow-ups filed: S139 (the chassis sweep back against props), S140 (the speed from the walk's measure).
