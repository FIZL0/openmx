# 1 — cab motion (FINAL)

Task B7. 2026-11-02.

## What I did

- Recorded the original live from S207's truck state with `watch/b7-truck-strike.toml` (new): the
  chassis and impact lanes, the bike's collision record (rider +0x7d0), and the cab car's node matrix,
  its +0x2e0 copy and its two cached motion answers (+0x260 velocity, +0x290 spin; S136).
  `context/capture/b7/strike.csv`, 120 rows, frames 8413..8532. It reproduces S207's wreck to the
  digit (vel 8.381516 on its wreck row, reason 2).
- Compared the port's contact resolve on the strike tick with the original's. Every contact used the
  cab (prop 277), whose velocity in the port was (32.654633, −3.1728482, 31.174992) with spin
  (0.0049, 0.0269, −0.2263). The original's was (32.654633, **−1.1912202**, 31.174992) with spin
  (−0.0094, 0.0205, 0.0179). x and z matched exactly; y and the spin did not.
- Printed the port's cab pose on every tick against the capture's `cab_pose`. The height and the up row
  differed from the first tick (tick 3: y 111.149 against 111.162, up.x 0.0375 against 0.0386). The
  original's up moves linearly, by the same step every tick: a blend of key normals, not a filtered
  ground query.
- Cause: `SaveState::traffic_of` built every car with `Spec::new`, which leaves `presample` false, so a
  free09 car (all of them have +0x4ec PresampleTerrain set, S128) took `world.ground` and the normal
  filter. Fixed in that function: a car with +0x4ec set and its first frame done (+0x4dc) is built as
  tests/presample.rs's `spec_of` builds one (the image's keys with their normals, the clip's lengths,
  the kept tangents as `measured`, `Spec::presampled`), and its `stale` comes from the dirty word
  (+0x4fc), as presample.rs's `car_of`. A car with +0x4ec set but no first frame yet gets
  `presample = true` and samples on its first step. Cars without the switch are as before.
- After the fix the port's cab pose equals the capture's on every tick printed (3..107), bit for bit.
  The strike tick (105) agrees on velocity, quat, cofg and every other lane but `ch_xform.m30`, origin
  x: 0x452f0e6e original, 0x452f0e6f port. With the origin lane left out of a debug run, truck parts on
  106 (the thrown quat), as rails does on 99.
- The S207 test now checks that the port's velocity on a matched wreck tick equals the capture's bit for
  bit (truck and rails). The agreement tick in `S207_RIDES` stays 105, since that column counts the
  origin lane.
- S219's trailer row (free09, `traffic_with_trucks` too) now agrees over the whole capture; it parted on
  45. Its ratchet is now `u64::MAX`. To run it I copied s2's `context/capture/s219/trailer.csv(.json)`
  and `context/states/s219-trailer.eeMemory.bin` into this slot's `context/`.

## What the original does

A free09 path car (S128) presamples its keys on its first frame and never queries the ground again.
Each tick, its height is the curve's y and its up is the segment's two key normals blended at the
curve's parameter. So its pose, and the velocity and spin its virtuals answer (S136), are those of the
presampled path. The port's `traffic.rs` already did this (tests/presample.rs proves it); only the
replay's seeding from a state had not set the switch.

## How it was proved

- A PINE recording compared frame by frame: `context/capture/b7/strike.csv` (120 rows) against the
  port's cab pose and strike contacts.
- The bit-exact replay fixture: `cargo test --release -p game-sim --test replay`: 115 passed, 1 ignored.
  `a_path_car_into_a_parked_bike_wrecks_it_as_the_original_does` (truck parts on 105, origin.x by an
  ulp; its wreck tick's velocity bit-equal) and `a_bike_meets_a_moving_truck_as_the_original_does`
  (trailer agrees on every row; the other three S219 captures are not in this slot and skipped).
- Checked against HEAD's test file: the trailer row parted on 45 there (vel.x 0.98477787 vs 0.9887709).

## Not verified / not 1:1

- The strike tick's origin.x, one ulp off (S261): the same wreck-tick ulp as S207 rails, S162 bump,
  S191 headon and S219 flank. The port's origin is integrate's own formula, the one bit-exact on every
  tick before. On the wreck tick the original writes something else into the transform's translate; not
  found yet.
- The thrown bike after the strike (tick 106 on) is not 1:1: the quat parts at once (0.0719 against
  0.0367). Not looked at.
- The bike's collision record in the capture (`rec_count`, `c0..c7`) read 0 on every row, the wreck row
  included, so the contact lists were not compared entry by entry. The resolve was compared through
  its result, which is now bit-exact. The watch spec's record lanes may read the wrong word, or the
  record is cleared before the frame ends.
- The `PresampleTerrain` car with its first frame not yet done (`presample = true`) is not used by any
  state replayed here; it is untested in this path.
- S219's flank, slow and rear rows were not run in this slot (no captures here); the free06 cars they
  use do not have +0x4ec set, so the change does not reach them.
- Recording this capture overwrote copy 3's savestate slot 40, a state another batch had written there
  at 11:07 today. It cannot be recovered (its `.backup` is from 10-27). The slot now holds S207's truck
  state.

## Next

Nothing for B7. S261 is the wreck-tick origin ulp.
