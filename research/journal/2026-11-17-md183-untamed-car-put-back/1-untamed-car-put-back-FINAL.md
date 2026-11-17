# 1 — untamed car put back (FINAL)

Task MD183. 2026-11-17.

## What I did

- `crates/game-sim/src/atv_car.rs`: the respawn's ground reset now reads `Vehicle::untamed`. With it on,
  it takes no collidable out of the database before the orient (Untamed's ground reset has no part
  unhooks). Everything else in the respawn is SLUS-21104's code, which Untamed's is at moved offsets.
- `crates/game-sim/tests/mxvatv_car_frame/untamed.rs` (MD161a's harness):
  - reads the respawn's words (`read_back`, through the Untamed view) into `Vehicle::back`, so the
    port's own respawn runs on the unwreck;
  - answers the put-back's ground query (`Call::BackGround`) with the original's answer to the same
    point (no nearest-query fallback on a put-back frame any more);
  - compares the respawn's words (`back_words`) on every frame, beside the car, body, pieces, vehicle
    and record;
  - lets a put-back frame hand out the put-back's calls (ground query, ghost start, trail reset, node
    unhook/rehang, collidable on/attach);
  - frame 83 of the wreck drive is now compared; the wreck tests assert it is the one put-back frame.
  - counts a handed-out `StandsOn` (MD131's ask, which the port makes before any tire step since
    MD131 landed beside MD161a) as unported, like the tire step was. Without it the throttle tests
    failed on HEAD at frame 161. The skipped frames are MD161a's, unchanged: 161, 258, 259, 260, 268
    (noted on MD233, which answers them).
- No `../opentennis` checkout on this machine; a tennis port has no car put-back anyway.

## What the original does

- Respawn (`./kit fn 0x004e8510 -p game`, Untamed scratch config as in MD93c's memory note): the
  put-back with (car, 1, 1), then with the engine and all four tires, each tire's reset and the engine's.
- Put-back (`./kit dis 0x004ca900 --len 424 -p game`): instruction for instruction SLUS-21104's
  `0x004a6f70` at moved offsets (spot +0xd30, rows +0xd40/+0xd50, look words +0xd60..+0xd68 to the
  camera at +0x6a4 +0x94, terrain +0x3e4, `vmulx.xyz` / `vadd.xyz` by 0.3, ghost +0xdd8/+0xddc, trail
  +0x10cc, post (8, 0x15, +0xd10, 0) on car+0x698).
- `0x004c8880`: ground reset `0x004b9130`, then +0x34c = 1 (the latch).
- Ground reset `0x004b9130` (`./kit fn 0x004b9130 -p game`): node unhook with +0x75 bit 1; body vt +0x14
  (staged velocity 0,0,0,1); orient vt +0xf0; rehang with that bit; with +0x74 bit 0 each collidable
  on and attached (+0x100, then +0xfc and +0x104). **No part unhooks**: SLUS-21104's `0x00492ca0`
  unhooks +0xd0, +0xcc and +0xd4 first. That is the only difference the port needed.

## How it was proved

- **Bit-exact replay** of MD160's wreck drive (context/capture/md160/wreck, tape in
  context/recordings/md160), SLUS-21701's frame in the interpreter beside the port:
  - `untamed_wreck_frame_beside` and `untamed_wreck_ride_beside`: 415 frames each (was 414), 934,174
    words, frame 83 (the put-back) among them, every word equal.
  - On frame 83 the port asked the ground at exactly the point the original did (no fallback), and the
    original asked nothing the port did not.
  - Handed out on frame 83: BackGround 1, GhostStart 1, NodeUnhook 1, NodeRehang 1, PartOn 3,
    PartAttach 3, TrailReset; no PartUnhook (with the switch off, the harness rejects the PartUnhook calls).
- The whole `mxvatv_car_frame` binary passes, MD154's SLUS-21104 respawn tests among them (the switch
  is off there).
- Only Untamed's switch changes the port, and nothing outside tests sets `Vehicle::untamed`, so every
  existing replay hash is unchanged.

## Not verified / not 1:1

- **The put-back's handed-out calls are listed, not checked against the original's calls.** The ghost
  start (vt +0x3a0), trail reset, node unhook/rehang and collidable on/attach run inside the original's
  frame; the harness only checks the port hands them out, and that every compared word after the frame
  is equal. Their arguments and order are not compared (MD154 does that for SLUS-21104 only).
- **Two of those calls take other arguments in Untamed**: the node's rehang is handed (2, 1) where
  SLUS-21104 hands (1, 1), and the collidable attach is a six-argument call (…, 1, 0, 0) where
  SLUS-21104's takes four (…, 1). The port's `NodeRehang` and `PartAttach` carry no arguments, so a
  caller answering them for Untamed must pass Untamed's; nothing in the port answers them today.
- **The put-back post (8, 0x15) is a `Cue(21)` handed out**, not checked against the original's event.
- **The spot words (+0xd30..+0xd6c) still come from the capture** each frame, as in MD161a: Free Ride's
  script keeps them outside the sim frame, and that script is not ported.
- **One put-back only**: the wreck drive's frame 83, with a terrain and the look words set. The no-terrain
  branch and the look words' zero tests are not reached by a capture; they are SLUS-21104's code, which
  MD154's 2,000-case oracle covers, at Untamed's offsets.
- **Untamed's orient (`0x004b92c0`) and body commit (`0x004b94e0`) are taken as SLUS-21104's**: not read
  instruction by instruction here; the one put-back frame matching bit for bit is the only proof.
- **Throttle frames 161, 258, 259, 260 and 268 are still not compared**: a tire stands on a loose prop
  and nothing answers `StandsOn` for Untamed (MD233).
- **The ground query's answer comes from the original's own call** (MD177 for the port's terrain read).

## Next

Land it (`kit done`).
