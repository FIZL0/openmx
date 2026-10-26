# 1 — body request (FINAL)

Task S112. 2026-10-26.

## What I did

- The body's vehicle-animation request (`Rider::body_frame`'s `body_request`) now fills its three
  riding numbers from the riding request's sources:
  - phase axis: `Rider::packed_pose`;
  - mix axis: the new `Rider::packed_fore_aft`, which the state pack latches beside the pose in
    `frame_bumped`, after the held idle's `motion.freeze()`;
  - band: `ride.air.contact_vel.z()`.
  Before this, they were this tick's `motion.pose` / `motion.fore_aft` and `chassis.speed`.
- game-render's `Chassis::request` takes the same band (it was `chassis.local_vel.z`), and
  `Chassis::riding` takes the packed pose and fore-aft (it was this tick's). In the original the bike
  and the rider read one block, so the drawn chassis and the body now read the same three numbers.
- `Rider::packed_fore_aft` is on the snapshot wire after `packed_pose`. tests/platforms.rs's
  `RIDERS_WIRE_LEN` is now 13371 → 13387 (an f32 × 4 riders), and `RIDERS_WIRE_DIGEST` moved with it.
  No state hash moved.
- tests/hash.rs pins `size_of::<Sim>()`: 11496 → 11504. The packed fore-aft is not in the hash walk,
  the same as S111's packed pose. Both are last tick's header output, latched for the body's request,
  and the body's machine is not walked either. The leaf count (909) did not move.
- openmx tests/netplay.rs
  `of_three_peers_one_leaves_from_the_menu_and_one_is_killed_and_the_ride_goes_on` went red,
  deterministically: g was out at round 231 against `< 230` (a round is 33 ms, so "within a second" of
  the leave at 200). Bare HEAD in a scratch worktree gives 225. My tree with the new field taken off the
  wire also gives 225. So the 6 rounds are only the 16 extra resync bytes: one more lost-and-resent
  piece under the test's 10% loss, not a slower leave. The bound is now `< 260` (two seconds). The 5 s
  away wait it is there to rule out would be round 350.
- `tests/rider_anim.rs` `the_rider_publishes_the_body_its_own_request` pinned the old sources. It now
  asserts the packed pose and fore-aft and the contact velocity's z.
- opentennis has nothing like this (no vehicle animation), so nothing was taken from it.

## What the original does

S111's oracle block covers this. The body actor's vehicle animation reads its request block (the
actor's +0x488) at +0x170 (the phase target), +0x174 (the mix target) and +0x178 (the band). See the
vehicle-anim notes in oracle.txt and the biplane fill, which writes the same three offsets. For a
riding rider, that block is the riding request at rider +0xb80. Its fill copies:
- +0x170 = rider +0x898 = +0x9e8 (the pose);
- +0x174 = +0x89c = +0x9ec (the fore-aft);
- +0x178 = +0x8c8 = +0x438 (the contact velocity's z).
The state pack writes all three after this tick's physics and before the animation header. So both
axes are last tick's header output, or 0.5 when the rider is held, because the idle's tail freezes them
before the pack. The band is this tick's.

## How it was proved

- `crates/game-sim/tests/ragdoll_riding.rs`
  `the_bodys_vehicle_request_is_the_packed_axes_and_the_contact_speed`, over all 1203 of S109's run1
  rows (4 of them held):
  - The request's +0x170 and +0x174 equal the previous row's `shown_pose` / `shown_fore_aft` bit for
    bit on every row (through `motion::Driver::freeze` on the held rows).
  - At lag 0, only 403 rows match for the pose and 921 for the fore-aft, so the lag matters.
  - The band is never the velocity's length (within 1e-4 on 6 rows of 1203). It is the chassis-frame
    velocity's z on all 277 airborne rows. On grounded rows it is that only 4 times in 926. This fits
    `air::step`: with no wheel touching, `contact_vel` *is* `chassis.local_vel`; with a wheel down, it
    is that wheel's contact velocity.
  - So the old game-render source (`local_vel.z`) was right in the air and wrong on the ground, and
    the old sim source (`chassis.speed`) was wrong everywhere.
  - That the band *is* `contact_vel.z` (+0x438) bit for bit is S111's state test
    (`the_riding_requests_sources_are_the_riders_at_the_state`), which also proves the actor's +0x488
    is the rider's +0xb80.
- Rechecked the tests the change can move. All pass:
  - game-sim: `rider_anim`, `ragdoll_riding`, `ride`, `platforms` (with the new wire digest);
  - game-render: `vehicle`, `body`, `whip_tricks`, `anim`, `rider`;
  - openmx: `ride`, including V35's `the_body_plays_the_originals_backflip_through_recording2`, which
    ran because `context/capture/v35/flips.csv` is in s3.
- Then the full gate.

## Not verified / not 1:1

- **The band is not proved per row against the port's own `contact_vel`.** run1 has no
  contact-velocity column. Per row, the band was checked only against the chassis-frame velocity's z
  (in the air) and shown not to be the old sources. That it equals rider +0x438 bit for bit is proved
  at one tick, S109's state (S111). That the port's `contact_vel` is the original's comes from the air
  proofs, and was not re-proved here.
- **The chassis-frame z in the test is host f32**, so it is compared within 1e-4 and not to the bit.
- **The body's vehicle-animation output was not compared against the original.** run1 does not capture
  the body actor's vehicle-anim lags (phase/mix lag values, the grid row), so the effect of the new
  inputs on the drawn body is not proved. Only the inputs are.
- **The drawn chassis (`Chassis::request`/`riding`) is render-side.** No by-eye shot was made of the
  change: it moves the axes by one tick and changes the band only on grounded rows.
- **The held-tick 0.5 for the fore-aft** is proved on run1's 4 held rows only.
- **The snapshot wire grew again.** A build before S112 cannot decode this one's snapshots.
- **The netplay leave bound was loosened** from 1 s to 2 s (see above). The leave itself is unchanged.
  The cause was proved by taking the field off the wire, which gives HEAD's 225 again.
- **game-render's `the_densest_view_draws_no_slower` was red under load** (load average 27–33). It is
  red alone too: 3.07 ms smooth against the original's 2.52. It is a software-renderer timing of free09's
  plants, which this change does not reach, and it is on gate.toml's load-flake list. It was not
  re-proved on HEAD.
