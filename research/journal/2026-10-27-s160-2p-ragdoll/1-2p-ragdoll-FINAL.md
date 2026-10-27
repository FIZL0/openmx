# 1 — 2p ragdoll (FINAL)

Task S160. 2026-10-27.

## What I did

- `every_2p_wrecks_bones_are_the_originals` in crates/game-sim/tests/ragdoll_wrecks.rs. It covers
  C10's 17 wrecks of the 2p recording, for both riders. Each run starts from S155's RAM image before
  the wreck (context/states/{c10,s155}) and goes to ten rows past the capture's put-back (mode 0).
  Each tick, the original runs its player update and its body update. The port's
  `ragdoll_body::Actor::tick` runs beside it **on its own state, carried from tick to tick**.
  From the original it gets only the world's answers (ground, sphere tests, sweeps, the skeleton's
  pose: S61's `port_tick` listeners) and the vehicle animation's two outputs (below). On every tick,
  every number the update writes is compared bit for bit: every bone, pin, joint, collision and actor
  number, the draws, and the tail handed to the request.
- To reach a 2p body, S56's fixed actor address became a thread-local (`body_actor()`, `with_actor`).
  The S56/S61/S62 tests set nothing and keep 0x017a7700. That let the new test run its 17 runs on 6
  threads (14 s).
- The put-back: the ride machine's own request is hooked and listed. The fader is not run, so the
  page's listener (the respawn) is called at the end of the tick before the capture's put-back row.
  On P1's three chord wrecks, the human's L1+R1 is played on the put-back row.
- research/oracle.txt: the S160 block (2p body hops, the tick's order, what writes the actor, the
  put-back and the chord).

## What the original does

- A 2p rider's body actor is the game manager's list entry after the two bikes, less 0x400. The list
  order is P2 bike, P1 bike, P1 body, P2 body. A tick is the player update (`./kit fn 0x0027c890`, on
  the bike), then the body's update (`./kit fn 0x0024e640`), both on the dt of the row before (S155).
  `bike_vehicle_update` alone leaves the body a tick behind its bike on every riding row. Some images
  were saved between a body's update and its bike's. Their bones are the next row's, so the player
  update runs alone first.
- **The body's update runs the vehicle animation** through the actor's vtable. The posing flag
  (+0x680) and the clip asked for (+0x8f4) change inside the update. On the NoHander entry at P1
  133954 they go 1 → 0 and 5 → 40. The body's mode reads the flag on the next tick. The port's
  `Actor::tick` does not run the animation (vehicle_anim.rs's `Update` does). A carried port that
  keeps the pre-wreck `posed` skips `feet()` at the put-back: holds 2–3 stay off, collisions 0–1 stay
  on, and the riding entry's ground queries are never made. All seven carried runs that parted before
  this fix were trick wrecks (body mode 3 before 2).
- **Nothing else writes the actor between two body updates**: not the player update, not the
  ride machine's put-back request, not the page listener's respawn, not the chord. The test checks
  this on every tick (the original's actor after its update == the actor before the next).
- The ride machine asks for the put-back (`./kit fn 0x0027b9f0`, kind 1, page 1) 5–6 rows before the
  capture's mode 0 for P1 and for P2's 134315 and 140363, and 9–10 rows before for P2's 131595 and
  133800. The viewport fader takes the rest. The human ended P1's 128995, 129945 and 130966 sooner
  with L1+R1 (pad byte 3 `0xb3`/`0xf3` on the put-back row): kind 1, no page, at once.
- Riding again (mode 1) comes 2 rows after the put-back on every wreck.

## How it was proved

RAM-image runs of the original's own update with hooks:
`every_2p_wrecks_bones_are_the_originals`, 17 runs, 2,824 ticks.

- **Port == original on every tick of every run**, the port carried on its own state through the
  entry pose, the flight, the ground contacts, the rest, the put-back and the riding entry. Over the
  runs: 15 entries, 15 off the bike, 2,166 touching ticks, 294 flung, 9 landings, 14 settles, 1,482
  settled ticks. 128995 is put back by the chord before it settles.
- 15 of the 17 wrecks are entered, put back and ridden on again, each 2 → 0 → 1 as the capture goes
  (floor `TWO_P_RAGDOLLS`). Five P1 runs (128995, 129945, 130966, 133313, 134381) hold every captured
  bone bit for bit, from the image to the wreck row (floor `TWO_P_BONES_TO_THE_WRECK`). 21 of the 30
  wreck-entry and riding re-entry switches begin on the capture's own row
  (`TWO_P_SWITCHES_ON_THE_ROW`). The 9 off it are listed by the test: eight wreck entries a row early
  and 128995's riding a row early.
- Mutations, each red, then put back: the bones floor at 6 ("only 5 runs held the captured bones to
  the wreck (was 6)"); the vehicle animation's `posed` not handed to the port ("P1 133313: the port's
  ragdoll parts from the original's on row 133475 … 6 of the original's ground queries the port did
  not make"). The whole ragdoll_wrecks file passes (13 tests) with the actor-address change.

## Not verified / not 1:1

- **The runs' bones are the capture's only up to the wreck, and on P1's five runs only.** From the
  entry tick on, the original's own run parts from the capture's bones by ulps to ~1e-3 ft/s (e.g.
  a bone0 velocity x of 54.4904 against the capture's 54.4887). After the put-back they never come back to it.
  The other 12 runs part before the wreck. So "port == original" is proven on the original's run from
  each image, not on the recording's exact numbers. Reason: the frame does work outside the two
  updates (S155's finding). Updating both riders in list order did not help. Unblock: S176, the whole
  frame from each image.
- **P2's two Scenery wrecks (130418, 130897) are never entered**, so no ragdoll runs there (S155:
  the runs do not reach reason 6). S176 too.
- **Eight wreck entries come a row before the capture's**: P1 135196, 135855, 136559 and 139659, and
  P2 131595, 133800, 134315 and 140363. These are the runs that left the capture before the wreck,
  plus P2 images already on the wreck row. The P1 ones match S155's "a row early". 128995 rides
  again a row early (129063 against 129064) because the chord is played before the updates, not
  where the input event really runs.
- **The fader is not run.** The respawn is placed on the capture's row (the end of the tick before
  mode 0), not reached through the fade, so the put-back row is the capture's by construction and is
  not counted. What is proven is that the original's ride machine asks for it, and what the body does
  from there.
- **The chord is emulated**: the latch at bike+0x14d0 set and `ride_reset_request(bike, 1, 0)` called
  ahead of the player update. The input event handler's own path is not run.
- **The vehicle animation's +0x680/+0x8f4 are handed to the carried port from the original** after
  each update, as a world answer, not computed by the port. Whether vehicle_anim.rs's `Update` gives
  the same two numbers in the 2p ride is not checked here.
- The 2p ride has no rider-vs-rider, truck or prop contact (C10). The ragdoll's hits on another
  collidable are never exercised (0 sphere hits with another collidable in all runs).
- The images and the capture are in s3's context/ only. Other slots skip the test with a line.

## Next

S176 (the whole frame from each image) would let these runs hold the capture's bones through the
wreck, and reach P2's Scenery wrecks.
