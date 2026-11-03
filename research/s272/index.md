# S272: C16's flying tape, by S228's method

The tape is C16's `context/recordings/c16/fly.p2m2`, with its state, which holds frame 137402. The
capture is `context/capture/c16/fly.csv`: one row a tick at 60 Hz, 137403..140224. Both were copied from
slot s2. The events are the plane-0 events in research/c16/index.md.

The test is `crates/game-sim/tests/c16_fly.rs`. Run it with
`cargo test -p game-sim --test c16_fly -- --nocapture`. It prints each run's holds and parting rows.

## RAM images

There are 39 images, `context/states/s272/s272-<frame>.ram`. Each is the original's RAM at the end of
that frame, and image() asserts frame_counter.

- Ticks: research/s272/image_ticks.txt. These are every 100 frames from 137410 to 140210, plus 137845,
  137880, 138020, 138050, 139190, 139355, 139490, 139850 and 139900. 137402 comes from the state itself.
- How they were made: PCSX2 copy 3 ran
  `./kit pcsx2 with -- bash research/c10/play_at.sh <pid> context/recordings/c16/fly.p2m2 10 <ticks…>`,
  then `KIT_PCSX2=3 python3 -B research/s272/rams.py <its log>` converted the saves.

## Timing

Tick t polls tape field t − 137404, which is the state's frame + 2. It steps on row t+1's dt (FIELD_ZERO;
`#[ignore] the_tape_offset_the_taxi_holds_longest_under` checks the offsets). The image's frame is the
seed, and row t+1 normally reads the state after tick t. Around the joust the rows lag one frame more:
the port's state after 137855's step is on row 137857.

## The events

Plane 0 seeded off the image before each event, compared bit for bit with `held()`:

| event | run | holds |
|---|---|---|
| the switch and taxi | 137402..137845 | 443/443 |
| the joust on P2, Sim plane lane (P2 standing still) | 137845..137900 | 8/55 (parts on the hit) |
| the joust, split-screen pass (`fly_two`): plane / P2 | 137845..137900 | 11 / 55 of 55 |
| take-off at 138061 and the climb | 138050..138200 | 150/150 |
| inverted ground crash at 139204 | 139190..139260 | 70/70, crash 1 on 139204 as in the original |
| down inverted, through the put-back | 139355..139560 | 205/205 |
| the 300-tick put-back at 139504 | 139490..139560 | 70/70 |
| 46 ft/s on static scenery at 139913..139918, no crash | 139900..139960 | 60/60, no crash |

Run in 100-frame stretches from each image, the whole tape holds except two stretches: 137810's
(43/100, the joust's hit in the Sim lane) and 137910's (0/100). The pins are in STRETCHES.

## What was fixed

1. **P2's joust wreck.** The fix is `rider_contact::wreck_contacts`, called at rider.rs's wreck test.
   - What happened in the port: P2's body ray reached 4.58 on plane 0's hull, past Race500P's 50cc hit
     limit of 4.0. But `mover_contact::wreck_contacts` keeps only scenery entries (other None), so the
     entry naming the plane (other = players + k, i.e. Bump::planes[k]) never reached `air::hit_wreck`.
     P2 rode on.
   - The fix: own entries on a flown plane's hull now go to the wreck test as
     `air::Other::Object{mass: plane mass 46.58, vel: plane body velocity}`. That is a Joust (7) for a
     bike lighter than the plane.
   - Result: P2 now wrecks 7 on the original's tick, and its chassis holds through the wreck (55/55).
2. **Seeding a plane off an image taken in flight or during a crash.** No record word holds these three
   things, so the seed builds them as the port's own steps leave them:
   - **The aero model's true speed.** Flight::post sets it from the last frame: a wheel down, or the
     record touching. A fresh Flight has it on. In the air the yaw damping must read the floored speed,
     so every image taken in flight parted on its first tick by a few ulps of velocity.
   - **A wreck's gravity** (Flight::wreck: WreckGravity × mass). Without it, images during a crash
     parted on their first tick in vy (about −0.2 ft/s).
   - **The game's rand()**, from `rand_state`, which the touchdown and throw picks draw.

   I found these by diffing the whole Flight at 138110 two ways: as the run from 138050 left it, which
   holds, and as seeded off the 138110 image.

## Where it still parts (filed)

- **After P2's wreck, the plane parts one tick later.** The port's vy at 137857 is +0.0302; row 137858
  reads −0.0535. The original's wrecked bike goes on meeting the hull in a way the port's chassis, frozen
  at −31.82 ft/s, does not. The 137910 stretch is the same thing: the plane bounces on P2's wreck, and
  P2 is seeded standing, with no wreck state.
  S274 (research/s274): the wreck bike's rays strike the hull, and their mirror on the plane's list
  is what moves it. Ported in `game_sim::wreck_plane`. From 137910 the port's plane after the first
  substep is row 137911 to its digits: that row is torn (read mid-frame), and `held()` wants the tick's
  end, so the stretch "parts" on a row it matches. The first tick after the wreck (137856, row 137858)
  still matches no state of the port's; the wreck bike's seed is the suspect (follow-ups filed).
- **P2 standing from 137810 parts at 137827**, 17 ticks before the plane reaches it. P2 is seeded from
  its chassis only (standing_p2, the way fly_and_collide seeds P1). Its rider and suspension state is
  not read.
- **The Sim plane lane never steps the other bikes** (flight_search::Bike stands still). Only the
  split-screen pass reproduces a joust.
- **Traffic (car0..5 in the capture) is not modelled.** No event here meets a car.
