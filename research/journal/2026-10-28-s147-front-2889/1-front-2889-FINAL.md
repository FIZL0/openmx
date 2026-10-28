# 1 — front 2889 (FINAL)

Task S147. 2026-10-28.

## What I did

- Looked at front's capture round 2889 first (bone 0's `last_pos`/`pos` per row). Row 2889's bones
  equal row 2888's exactly, but `rng_state` differs, so `repeated` (which needs the generator unchanged
  too) does not skip it. Row 2890's `last_pos` is not row 2889's `pos`: two steps lie between the rows.
- Diffed every laid-in column over 2885..2892. Row 2889's `req_area`, `bike_a`/`bike_b`, `req_far` and
  the world blocks are all row 2888's. The race clock block (`w05ca1f8`) also stands still at 2889 and
  then moves 2 dt at 2890. So the whole row was read before the tick's simulation. This is the
  frame-counter tear (the recorder wakes on `frame_counter`, stepped before simulate; memory
  "frame_counter-triggered captures tear"), not a lag frame.
- Ruled out a collidable: the hidden tick (and every tick from 2884 to 2899) makes no ground query, no
  sphere test and no sweep. The bike has been out of the broad phase since the put-back (2681, S108),
  and the bones are pulled by the pose toward the request's frames and points only.
- The hidden tick's request: laying in 2888's (= 2889's) leaves bone 0's first step at x 4183.3623,
  laying in 2890's at 4183.254, and the capture's 2890 `last_pos` is 4183.3076, between them. Neither
  row's request is the tick's, and none can be: the tick's own request was never read. Also ruled out:
  draws before the body (it draws none), and one call with 1.5×–3× dt (no match).
- Laying row 2890's six bone quads (`BODY`) into the Ee after the double step brings back the
  original's own update, bit for bit, at 2891. The same holds after 2892 and 2894, through the last row
  (3180).
- Tests (crates/game-sim/tests/ragdoll_wrecks.rs):
  - `torn` (row read before the body ran) and `lay_bones` (a row's bones into the Ee).
  - `S62` front's bound is now 3180, the capture's last row (was 2888).
  - `the_port_replays_the_wrecks_through_the_bike` handles torn rows. A torn row and the row after it
    are the original's ticks on the capture's request, compared with the port (`port_tick` without
    `drew`) but not with the capture. Then the next row's bones are laid in and the port takes the
    original's actor again (`port = None`). It asserts 3 gaps and ≥2500 capture-exact ticks (2576 now,
    was ~2290).
  - `every_sphere_test_of_the_wrecks_is_the_bike_meshs` handles torn rows the same way. Its counts are
    unchanged.
  - New: `front_2889_is_a_row_read_before_the_tick`. It checks that the torn rows are exactly front's
    2889/2892/2894 and none in side/back/tip/front3, that row 2889's laid-in columns and clock are
    2888's, and that the clock moves two ticks to 2890. It checks that the hidden tick draws nothing,
    that 2888's and 2890's requests bracket the capture's first step (bone 0 x), and that 2891 holds
    with 2890's bones laid in.
- Took nothing from opentennis: it has no ragdoll capture replay.

## What the original does

Nothing new about the game. The body's update past the put-back reads the request (rider frames at
req+0x210..0x288, velocity +0x290, the hand/body points from +0x2b0) and steps the pose toward it; no
collision is made. The main loop steps `frame_counter` before it simulates, so a PINE read that lands in
between shows the tick before under the new counter.

## How it was proved

- Bit-exact replay: the original's update (`rider_body_update` in the Ee) from front-2602's RAM image.
  Every captured bone lane (18 bones × 6 quads) matches on every compared row 2603..3180, 572 rows, with
  only the 6 torn/double rows uncompared. The port matches the original's update on all 578 rows,
  including the 6.
- The cause, by `front_2889_is_a_row_read_before_the_tick` (above).

## Not verified / not 1:1

- **The six front rows 2889, 2890, 2892, 2893, 2894, 2895 are not checked against the capture.** The
  first tick of each double step ran on a request the recorder never read, so neither the original's
  update in the Ee nor the port can be given its world. The port is still compared with the original's
  update on those rows, on the torn row's request, which is not the game's.
- **Laying in a row's six bone quads is assumed to be the whole of the body's state the next tick
  reads.** It is shown only by the ticks that follow holding bit for bit (2891, 2896..3180). It is not
  shown field by field. Actor fields the hidden tick wrote are the Ee's, from a wrong request. They
  agree with the game's as far as every later tick holding shows, and no further.
- Not re-captured: S62 found that reloading front's state does not ride the same wreck (time-based pad),
  so a clean row 2889 cannot be recorded again.
- Side's bound (3146) is untouched. Its 3147 is a `repeated` row (generator unchanged). It may be the
  same kind of hidden tick, but that is not this task's and was not looked at.

## Next

none
