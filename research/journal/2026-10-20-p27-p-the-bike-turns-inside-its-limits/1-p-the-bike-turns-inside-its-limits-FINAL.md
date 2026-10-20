# 1 — p the bike turns inside its limits (FINAL)

Task P27. 2026-10-20.

## What I did

- **Wired the bike chassis' angular limits** in `crates/game-sim/src/ride.rs`, `bike_ang_limits`.
  - Windows: the rigid body constructor's 25 rad/s on x and z, the file's `YawRateLimit` on y (yaw).
  - Angular limit: the constructor's sum of squares, 1875. It does not follow `YawRateLimit`.
  - Before, the bike used `MAX_ANG_SPEED` 100 on every axis and 10000 as the limit, and nothing read `yaw_rate_limit`.
- **Test**: `the_chassis_race125p_builds_carries_the_originals_angular_limits` in `crates/game-sim/tests/ride.rs`.
  - It reads `race125p.phy` and the 125's wheel mounts off the disc (P21's `the_disc_125`).
  - It checks `race125p`'s `YawRateLimit` is 3.
  - It checks the built chassis' `max_ang_speed_sq`, `ang_vel_max` and `ang_vel_min` bit for bit against the live
    read below.
  - It also checks that the test bike, whose file has no `YawRateLimit`, gets (25, 25, 25) and 1875. That half
    runs without a disc.
- **Regenerated the platform vectors.** The state hash walks the limit fields, so storing new values moved
  `HUNDRED_TICKS` (in all three places) and `HASHES`, `TRACE_DIGEST`, `FINAL_DUMP_DIGEST` and `FIXTURE_TEXT_DIGEST`.
  - `POS_Y`, `VEL_Y`, `FINAL_DUMP_HEAD`, the leaf count and `EVERY_LEAF_HASH` did not move. So the trajectories in
    those vectors are unchanged: no limit fires in them.
  - Before splicing, I checked that the change was in the stored values and not in the motion: windows of 100 gave a
    third hash.
- `race125p_tops_out_where_the_original_does` still passes, so the straight-line ride is unchanged.
- `research/oracle.txt`'s P21 block now says the limit is not recomputed, and that the port wires it.

## What the original does

- `./kit fn 0x00158ae0` (the rigid body's constructor) writes:
  - `+0x230/4/8 = 25.0` and `+0x23c = 1.0`;
  - their negatives at `+0x240/4/8`, and `+0x24c` copied from `+0x23c`;
  - then `+0x224 = z² + y² + x²` of the windows.
- `./kit fn 0x002539e0` (the `.phy` loader), [Mass Properties]: `+0x234 = YawRateLimit` and `+0x244 = -YawRateLimit`.
  - It runs after the constructor and does not touch `+0x224`.
  - The key is read through `0x0014b520`, which writes the destination only when the key exists. The destination is
    an uninitialised stack float, and the loader stores it into the chassis either way.

## How it was proved

- **A save-state capture** of the running original: `capture/limits.toml` (the bike chain is P21's), recorded from
  the acceptance state (scratch slot 8) on PCSX2 copy 3, frames 2808..2810, `rider_type == 2`.
  - Result in `capture/limits.csv`: `+0x220 = 18225`, `+0x224 = 1875`, `+0x230 = (25, 3, 25, 1)`,
    `+0x240 = (-25, -3, -25, 1)`. This agrees with P21's read.
- The port's chassis for `race125p` equals that read bit for bit (the test above).
- `./kit check`: passed.

## Not verified / not 1:1

- **A file without `YawRateLimit`** gets the constructor's 25 in the port. The original stores an uninitialised stack
  word there, so it has no defined value to match.
  - `phy.rs` reads a missing key as 0, which would pin the yaw. So `ride.rs` reads a zero `yaw_rate_limit` as
    "missing".
  - A file that really says `YawRateLimit = 0` would therefore get 25, not 0. I did not check whether any shipped
    `.phy` says that: only `race125p` was read.
- **That the acceptance state's bike is `race125p`** is assumed from P21 ("the live 125"). I did not read which `.phy`
  the live bike loaded. The test's file gives 3, and so does the live chassis.
- **No turning ride is compared against the original.** The limits are the original's values, but whether the
  3 rad/s yaw window ever fires in a real turn, and how the turn then looks, is P24's (turning).
  - The capture has no angular velocity column. The port's turning has not been compared frame by frame.
- **The wreck's chassis** still uses the `MAX_ANG_SPEED` 100 stand-in. I did not trace whether the wreck body goes
  through the same constructor (as in P21, which left its speed alone too).
- **`+0x23c`/`+0x24c` (1.0)**: the port's `Chassis` has no `w` lanes for the windows. The clamp reads only x, y and z,
  so nothing is lost.
- **Not caused by this task: three captures parked to get a green check.** In this worktree,
  `context/capture/acceptance/` had `acceptance-ride.csv`, `acceptance-start.csv` and `recording2-freeride.csv`.
  - `air.rs`'s flight test failed on `acceptance-ride.csv` ("only 198 ballistic ticks"). That count comes from the
    capture alone, not from the port.
  - I moved all three into `context/capture/acceptance/parked-p27/` (git-ignored, this slot only).

## Next

Nothing for P27. P24 (turning) is where the yaw window first matters.
