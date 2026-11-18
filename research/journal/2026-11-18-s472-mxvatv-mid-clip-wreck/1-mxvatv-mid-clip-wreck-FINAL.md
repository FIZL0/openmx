# 1 — mxvatv mid clip wreck (FINAL)

Task S472. 2026-11-18.

## What I did

- Re-recorded S411's `spin2` and two new backflips that land short, in copy 3, at NominalScalar 0.15
  (load average about 15; put back to 0.3 after). Every row is there at the landing and the wreck.
  - `context/capture/s472/spin2.csv`: `research/s411/spin2.pad`, a right 360 landed with its clip at 2.
  - `context/capture/s472/flipw30.csv`, `flipw38.csv`: `research/s411/flipw30.pad` / `flipw38.pad`,
    flip.pad with the stick held back 30 / 38 ticks, not 50. Both land with the backflip's clip at 2.
  - Recipe: research/s411/play.sh as its header says, with `context/recordings/s472/NAME.p2m2` made by
    c15's tape.py, 300 rows. PCSX2 must start with `QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1`
    (and `WAYLAND_DISPLAY`), or play.sh stops at "No accessible app for pid".
- Port (score.rs): `AtvRows::aloft`, whether the bike was off the ground as the frame started. The
  backflip and front flip watches read it in place of this frame's airborne. rider.rs passes its
  `was_airborne`.
- Test (`tests/mxvatv_stunt_score.rs`): `a_backflip_landed_mid_clip_is_lost_in_the_wreck` (flipw30,
  flipw38) and `a_three_sixty_landed_mid_clip_is_lost_in_the_wreck` (spin2). `ride_the_port` now runs
  the rider's wreck entry (`Score::crashed`, the 360 and front watch dropped) on the frame the
  original's wreck count goes up, and feeds `wrecked` after it.
- opentennis: nothing to take (no stunt scoring there).

## What the original does

- **The touch frame.** The wheels touch (bike `+0x324` set) at the end of frame T. The clip and the
  node run on through T: the node is still 0x3d and grows one frame more. The backflip step
  (`./kit fn 0x004e4f20 -o atv`) reads `+0x324` before the frame's update sets it. So it reads the
  frame before, as it reads the node's heights (S411).
- **The wreck frame (T+1).** The step sees the ground with the move armed. It ends the move: stunt
  word `+0x1528` 0xff, and with the clip running it sets the clip to 0 and calls the wreck entry
  (vt +0x368, reason 0xb, or 0xf from the steep test). There is no stunt end. The wreck's cancel loses the
  node: no nodes, card flags 0x60 off, total unchanged, combo points 0. The 360 (spin2) wrecks on
  T+1 the same way.
- **The bank clock.** The wreck frame starts with the stunt word still 41/45, so the clock is set
  back to 0.2 and counted once (0x3e2aab8a, the same as on T). After that it counts down by dt only on
  frames where the wrecked bike's `+0x324` is set. It stops at or under 0 (spin2: 0x37a7a000, then
  0xbd087016). flipw30's bike leaves the ground at 0.033 and holds it 13 frames, until it is down
  again. The port's S411 rule already does all of this.

## How it was proved

Three live captures, compared on every row with the port's card (nodes, combo/scoring flags, total,
combo points, bank clock, bit for bit), as S411's flip2/spin3. Undoing the `aloft` change fails
flipw30 at 13813 (row 185): the port finished the node on the touch frame. `ride.rs`
(`md25_mx_vs_atv_rides_as_before`) and `score.rs` pass unchanged.

## Not verified / not 1:1

- When the port's own pack ride wrecks after a mid-clip touch is the port's rider/handling. The test
  feeds the original's wreck frame and runs `Score::crashed` there. Whether the port's wreck comes on
  T+1 was not checked: the pack ride's physics are MX Unleashed's, so no ride of the port lands the
  same way.
- The port's crash runs MX Unleashed's `wrecked` (finish then close), not `atv_cancel`. On these three
  rides the card ends the same as the original's. Other combos at the wreck (several nodes, a wheelie)
  were not captured.
- `wrecked` in the test is "the wreck count went up", from the wreck to the end of the capture,
  through the respawn. Nothing scores after the wreck in these rides, so it changes nothing.
- The front flip watch reads `aloft` too. That follows the backflip, with no capture (the front flip
  is the port's own).
- The start state is S411's (not state 4), as there.
