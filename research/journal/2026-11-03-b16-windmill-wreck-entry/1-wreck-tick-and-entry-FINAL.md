# 1 — the wreck's tick and the bones' entry (FINAL)

Task B16. 2026-11-03.

## What I did

- Copied s3's `context/capture/r14/` into s1's context, since each worktree has its own.
- Read the image's bike search (rider `+0x7bc`). It holds 23 entries, gathered with the probe at
  (4310.0, 212.8, 3315.9): that is where the bike was before the teleport, not at the windmill.
  The port's `seed_bike_search` gathers afresh at the bike's new place, which holds the windmill
  (7 entries).
- `tests/replay.rs`:
  - New helper `seed_bike_search_kept`. It gathers the port's list with the probe on the image's
    kept centre, so the first tick's substeps see what the original's did, and the tick's own
    gather, now out of the bound, finds the windmill.
  - `s211_ride_in` (S211's and R14's rides) uses the new helper.
  - `S211Tick` gains `bone_vel`, the wreck's main-bone velocity.
  - `the_windmill_wrecks_the_bones_with_the_bike` is rewritten from "the bones within 12 ft" to
    the checks under "Checked".

## What the original does (south-bones.csv)

- Row 2, tick 1: the bike falls, vy −1.3415346. That is 1/24 s of gravity, with no contact.
- Row 3, tick 2: the windmill's throw, (7.8011017, 18.841103, 133.37538), and reason 4. The body
  actor's riding word (`+0x2a4`) is still 1 and placed is 1. The main bone moves at
  (0.787, 0.8006, 0.80003) of the bike's velocity, so the bones had a riding step on the bike as
  thrown.
- Row 4: riding is 0, so the wreck entry, which clears it, ran on this tick. The main bone is at
  (7.22, 15.26, 117.81) ft/s.
- The same lag shows elsewhere:
  - S56's four captures: wreck_reason is a row ahead of a_mode 2 (front 2599/2600, side 2604/2605,
    back 2474/2475, tip 2472/2474).
  - S110's run2: verdict 3716, entry 3718, with riding ticks between them.

## What the port does now

- Port tick `t` is row `t + 1`, as in S211.
  - Tick 1: no wreck.
  - Tick 2: wreck 4.
  - Both ticks' velocities equal the csv's bit for bit.
- The Down is entered on tick 2, the verdict's tick, from the tick before's pose. This test runs no
  riding ragdoll: the drawn chassis that holds it is game-render's. So the entry falls back on the
  kit's pose at rest (S70). The main bone's tick-2 velocity is (−0.015, −1.41, −0.066), and on
  tick 3 it is 7.17 ft from the original's.
- R14's "velocity is the bike's" was `enter_from`'s no-history branch: a wreck on the first tick
  has no drawn frame before it, so every bone takes the bike's velocity. It does not happen with
  the tick right.
- In the shell the riding ragdoll runs. There, the port would hand over bodies stepped on tick 1,
  with the bike at rest, while the original's were stepped on the thrown bike. So the gap is the
  entry's tick, not the test's alone: B19.

## Checked

- `./kit check -- -p game-sim --test replay`: 118 passed.
- The new test's assertions:
  - ticks 1–2 velocities are bit-exact, and the wreck is on tick 2;
  - no cannonball in the original or the port;
  - the original's actor lets go a row after the verdict;
  - the original's main bone moves at 0.8 ± 0.03 of the bike's velocity on the throw's row;
  - the port's tick-2 bone has |vz| < 5 and is more than 2 ft off on tick 3. These two are pinned
    to fail when B19 lands.
- S211's `the_rustic_windmill_throws_a_bike_as_the_original_does` uses the same seeding and is
  unchanged: ticks 1–3, 8/14 and 42/85 all as before. Its image's list has no windmill either, and
  its bike only reaches the windmill on tick 3.

## Not verified / not 1:1

- **The wreck entry's tick (B19).** The port enters the bones on the verdict's tick. The original
  enters a tick later, after one more riding step on the thrown bike. Not changed here: it moves
  the entry for every wreck, and the entry tests (S70, S110, S262), the ragdoll wreck replays and
  the hashes all need rechecking with it.
- **The hand-over against b{i}.** This replay runs no riding ragdoll, and the image's 18 bodies
  (orientation, rates, actor state) are not seeded into one. So the csv's bone columns are not
  compared through the entry. They are only bounded: the 0.8 carry on row 3, and the port's bone
  pinned as at rest. S110's `the_wreck_enters_from_the_riding_bodies` still holds the hand-over
  itself bit for bit, given the bodies.
- **The original's ragdoll on tick 1.** The port's bone on tick 1 is the kit's riding pose, 0.93 ft
  from the original's riding ragdoll. It is not compared further.
- **Other teleported images.** Whether any other test seeds a teleported image's search afresh was
  not checked. Only S211's and R14's rides use `s211_ride_in`.
