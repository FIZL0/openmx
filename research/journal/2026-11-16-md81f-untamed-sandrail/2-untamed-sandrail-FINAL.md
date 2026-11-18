# 2 — untamed sandrail: the drives replayed

Task MD81f. 2026-11-18. Part 1 left the replay to a follow-up (MD175, renumbered MD176, then reused by
another task) because MD161a's Untamed car frame harness was not on main. It is now, so the replay is
done here. Part 1's pack, load test and shell test are carried over unchanged from wip/MD81f (that try
never merged: three failed merges, ledger 11-16 12:33).

## What I did

- **Harness opened to other captures.** tests/mxvatv_car_frame/untamed.rs: a drive names its context
  folder (`set`: MD160's `md160`, the sandrail's `md81f`); `u_capture_rows` is `u_capture_check` without
  its verdict, since this recorder's rows sit one frame earlier than MD160's.
- **The sandrail's scratchpad:** context/untamed/md81f/spawn.spr.bin, the spawn .p2s's Scratchpad.bin
  (also in kept-context/md81f).
- **tests/mxvatv_car_frame/untamed_sandrail.rs** (13 tests):
  - capture: SLUS-21701's own frame, run in the interpreter off the spawn image with the tape's input,
    against throttle, throttle-2, steer and jump.
  - beside: the port's `frame_untamed` and the port's Ride (`update.untamed`), each frame read from the
    original's RAM and compared word for word after SLUS-21701's step, on throttle, steer and jump.
  - on the port's terrain: the same through the Ride with every ground query and noise sample answered
    by the port (Rocksbury's terrain, MD234's).
- Nothing from opentennis: no shared code involved.

## What the original does

- Nothing new: the sandrail steps through the baja's class in SLUS-21701, and the port's Untamed frame
  already holds every word of it on these drives. No change to car.rs or vehicle.rs.

## How it was proved

- **Live capture, frame by frame:** every row of all four captures is SLUS-21701's frame in the
  interpreter, bit for bit (body, engine, four tires): 370 + 370 + 280 + 370 rows. The recorder reads a
  row before its frame (row 0 is the image); throttle's row 27 is read after it (the late read part 1
  saw against throttle-2). No row is torn.
- **The port beside the original:** throttle 354 of 370 frames, steer 280 of 280 (17 of them wrecked
  after the wall), jump 370 of 370 (over the supercross jump): every word the port keeps (car, body,
  pieces, vehicle, record, respawn) equals SLUS-21701's after each frame, through frame_untamed and
  through the Ride, and through the Ride on the port's own terrain and noise.
- Pack off: no shipped code changed (only tests and research files), so the gate's replays are the check.

## Not verified / not 1:1

- **Throttle's 16 frames with a tire on an object** (111, 112, 145–150, 155–158, 292–294, 299, 307) are
  not compared: since MD131 the tire step asks what a tire stands on, and the Untamed harness cannot
  answer it yet (the same limit as MD161a's truck drives). The frames after match again.
- **The shell's sandrail is proved against a Ride**, not against a capture: the shell test drives on
  free09 (MX Unleashed's world), the captures are on Rocksbury. The Ride is what the captures prove.
- The class update's `!(d <= 0)` clause is still not ported (as MD81d's baja): it parts only on a `-0`
  distance, which no drive met (every frame matched).
- throttle-2 is checked against the interpreter only, not beside the port (it is the throttle tape again).
- The jump drive's landing and the steer drive's wreck are compared word for word but the wreck's
  put-back never happens in the drive (no unwreck in 280 rows).
- Look, sounds, cameras of the sandrail: not compared (presentation, MD157's).

## Next

- none
