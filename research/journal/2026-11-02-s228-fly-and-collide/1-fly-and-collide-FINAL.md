# 1 — fly and collide (FINAL)

Task S228 (with P227 and S227). 2026-11-02.

## What I did

- **Captured the tape.** I played `context/recordings/fly_and_collide.p2m2` from its `_SaveState.p2s`
  on PCSX2 copy 3, at NominalScalar 0.05 with watch/2p.toml (research/s228/play_capture.sh, recon.toml).
  That gave context/capture/s228/fly.csv: 3522 rows, 136505..140026, one per tick at 60 Hz, no gaps.
  I also saved 44 RAM images (research/c10/play_at.sh, then research/s228/rams.py) to
  context/states/s228/s228-<frame>.ram: 10..13 frames before each event, and every 100 frames from
  136510 (research/s228/image_ticks.txt).
- **Indexed every contact** in research/s228/index.md, using research/c10/index.py through a symlink
  (context/capture/2p/s228-fly.csv) and research/s228/landings.py and summary.py.
- **Wrote crates/game-sim/tests/fly_and_collide.rs:**
  - `p1_rides_each_event_as_the_original` covers the nine events. It holds chassis velocity and
    centre tick by tick, the request axes, and the wreck reason and frame.
  - `p1_rides_the_tape_as_the_original` covers the whole tape in 100-frame stretches, pinned in `HELD`.

## What the original does

- **The state.** It is the C10 session's split free09, a 2p state at 60 Hz, holding frame 136504. Both
  biplanes stand parked for the whole tape (speed < 0.003 ft/s), P2 idles, and P1 rides.
- **Wrecks.** In every case the first row showing the wreck reason is two frames after the tick the
  port wrecks on. 137251 is reason 3, a landing crossup (hang 1.618 s, touchdown pitch -23.9, roll
  18.6). 138436 is reason 8, an airborne roll of -85.28. 139446 is reason 1: fore-aft g of -56.64
  against -18.33, from a body ray on a scenery wall at 59.2 ft/s. 139894 is reason 6, a body ray on a
  trunk at 19.2 ft/s.
- **Nose-first landings (S227).** The 137250 landing wrecks on the crossup clause, not on the nose.
  The 139063 landing (nose 62.6 deg down, 89% of the 70 deg limit; vertical g 45.4 against 66) rides
  out.
- **Pads.** The pad for tick t is tape field t − 136507. The capture's pad0_a column drifts a frame
  either way, so it is not the pad to ride on.
- **Rows.** Capture rows drift in phase: a row can repeat a state, skip one, be torn axis by axis, or
  carry integrate's rebuilt centre. `held` accepts the row sequence as a subsequence of the port's
  states within a window of 3.
- **Request axes.** p1_req_axes x/y are the rider's packed pose and packed fore-aft, latched in step.

## How it was proved

- A save-state capture against a bit-exact replay of the port's sim (`./kit check -- -p game-sim
  --test fly_and_collide`). Both tests pass.
- Nine events: every tick of each ride holds the capture's chassis velocity and centre bit for bit.
  The request axes match on every riding tick, apart from the first tick (the motion driver is not
  seeded), ticks the capture skipped, and ticks after a wreck. All four wrecks match the reason and
  the frame.
- 35 whole-tape stretches: the held tick counts are pinned in `HELD`.
- **The seeder fix.** The image seeder now seeds the air authority, preload and flip fields. Without
  them the stretches at 138910 and 139410 parted, and they hold all 100 ticks with them.

## Not verified / not 1:1

- **Flight was not checked.** flight.rs and rider_contact.rs plane contacts are not exercised,
  because nobody flies on this tape (C16).
- **The drawn rider's lean (P227's render half) is not compared** to the original's drawn joints
  (S228a). Only the sim's lean and fore-aft request axes are proved.
- **Preload partings (138907, 139384).** The force is the stick flick's speed, and the port samples the
  flick its own way (`ride::Flick`), because the original samples it from DMA waits on a hardware
  timer. I believe this is the cause but did not confirm it from an image of those ticks: the images
  were started and stopped after 2 of 19, because each replays at 0.05 speed.
- **Unexplained grounded partings** at 136624, 137462, 137637, 137943, 138010, 138179, 138212, 138311
  and 138729, and the put-backs at 138684, 139141 and 139746 (S228b).
- **Mid-wreck stretches** (137310, 138510, 139510, 139910) cannot be seeded: the seeder makes a riding
  rider, not a ragdoll. So the ragdoll after each wreck is not held here.
- **The wreck test was not judged call by call** against the original's. The original's whole frame
  from these images stalls on a hardware wait (call 0x133500, pc 0x198bf0) (S228c). The wrecks are held
  through the port's ride against the capture only.

## Next

Nothing for S228. S228a, S228b, S228c and C16 carry on.
