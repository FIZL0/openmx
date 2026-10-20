# 1 — p a top speed of its own (FINAL)

Task P21. 2026-10-20.

## What I did

- **Captured the original's terminal speed**, using race125p flat out from a standstill along Free09's x = 4620 lane. The
  capture is `context/capture/p21/flat-throttle.csv` (+ `.json`, git-ignored): 480 rows at 30 Hz, frames 3471..3950,
  taken with `watch/acceptance.toml` with the state removed, plus the chassis' and engine's own words (`capture/extra.toml`).
  **The kit's pad could not be used.** `/dev/uinput` has been root-only since the 2026-10-19 reboot (plan/HUMAN.md has
  the ask), so the input was a *generated* PCSX2 input recording instead: Cross at full pressure on every field, both
  sticks centred, written by `capture/make_p2m2.py`. It was played back over the accessibility bus with C6's playback
  scripts (`capture/flat_capture.sh`). The recording starts from a state I made on the lane, kit scratch slot 09:
  - Pause the acceptance state (slot 08) over AT-SPI.
  - Shift the bike to (4619.5, 136.5, 2319.7), facing +z, with `capture/tele2.py`. It moves every float triple within
    12 ft of the chassis' centre of gravity in the bike, chassis, actor and transform objects by one delta. Writing only
    the actor transform, or only the chassis position, was undone by the next frame.
  - Zero the chassis' velocity and accumulators, let it settle, then save.
- **The original's terminal speed is 119.86 ft/s** in chassis units: 3-D distance per second over the last 30 rows,
  where `ch_speed` reads 119.89–119.93. It still rises by 0.2 ft/s over the last 2 s.
  - It gets there in 5th gear, which is the top gear (shifts at rows 59, 75, 100 and 151 at 58/71/85/102 ft/s).
  - The engine is at 8739 rpm, under its 9500 rev limit, with the throttle at 1.0 on every row.
  - Wheel torque falls from 296 at the 5th-gear shift to 237 at the end.
  - The rigid body's own limit is 135 ft/s, and the bike never reaches it.
  - **So on the flat, the cap is drag plus the tire and wheel losses against what fifth gear's torque curve gives.**
    It is not the rev limit and not a clamp.
- **The port already tops out there.** race125p on a flat floor at the same 30 Hz step reaches 121.98 ft/s in 5th gear
  (+1.8%). P15's 121.8 on the shell's lane is the same number. The brief's "no recognisable top speed" came from P15
  comparing against **75.78 and 79.89, which are `bike_vel` numbers**. On every row of this capture, `bike_vel` is
  0.8 × the chassis velocity (ratio 0.7999999–0.8000000): 119.9 × 0.8 = 95.9, which is the capture's `bike_vel` at the end.
  **That is R6's "positions advance 1.25× velocity·dt"**: the 1.25 is the inverse of the 0.8 in the actor's copy of the
  velocity. P22 and the new P28 build on it.
- **Wired the one speed cap the port had as a placeholder**: the chassis' `max_speed_sq`, now 18225 (135 ft/s). This is
  the literal the rigid body's constructor writes, and the live 125's chassis holds it (read off the running original
  in the lane state). Before, it was `MAX_SPEED` 1000, "wide enough that no ride reaches them". Only the bike's chassis
  is changed: the wreck's chassis keeps the stand-in (see below). The angular limits are P27.
- **The pinned hashes moved** because `max_speed_sq` is part of the walk: HUNDRED_TICKS in three places, and the
  platforms vectors (HASHES, TRACE_DIGEST, FINAL_DUMP_DIGEST, FIXTURE_TEXT_DIGEST) were spliced from `print_the_vectors`.
  POS_Y, VEL_Y, FINAL_DUMP_HEAD, the leaf count and EVERY_LEAF_HASH did not change. That is right: the limit is a value
  the walk hashes, and no ride in those vectors reaches 135 ft/s.
- **Test**: `race125p_tops_out_where_the_original_does` in `crates/game-sim/tests/ride.rs`.
  - It reads `race125p.phy` and the `125_faisst` model's TireFront/TireRear mounts off the disc.
  - It rides a `Rider` flat out on `Flat` for 16 s at `DeltaTime::CAP_30`.
  - It asserts the last second's distance-per-second is within 5% of the capture's, measured the same way.
  - It skips, with a line, when the disc or the capture is absent.
  - Today it gives 121.98 against 119.86.

## What the original does

- The rigid body's constructor (`./kit fn 0x00158ae0`) writes:
  - `+0x220 = 0x468e6200` (18225 = 135²);
  - the angular windows `+0x230/4/8 = 25.0` and their negatives at `+0x240..`;
  - `+0x224` = the sum of the windows' squares;
  - and two words, `+0x218 = 933156.0` and `+0x21c = 250000.0`, that I did not trace.
- The bike's setup (`./kit fn 0x00252ff0`) builds the chassis through `0x0015c2c0` (which calls the constructor) and
  writes nothing over `+0x220`.
- The `.phy` loader (`./kit fn 0x002539e0`) sets `+0x234 = YawRateLimit` and `+0x244 = -YawRateLimit` from [Mass Properties].
- Live, on the 125 in the lane state: `+0x220 = 18225`, `+0x224 = 1875`, windows (25, 3, 25, 1) and negatives. 3 is
  this bike's YawRateLimit.
- `research/oracle.txt` has the lines (the P21 block).

## How it was proved

- **A PINE recording**, 480 frames, of the original on its own physics, driven by a generated `.p2m2`:
  `context/capture/p21/flat-throttle.csv`. Cross was held on 479/480 rows, dt was 0.03333 throughout, and
  bike_type/ctl_src were 4/1 on every row.
- The port side is the new test above, through `./kit check`.
- The 135 ft/s limit is the constructor's literal plus the live word. No ride in the capture reaches it, so its *effect*
  is not compared.

## Not verified / not 1:1

- **The ground differs.** The capture is on Free09's terrain, and the bike drifted from x = 4619 to 4577 over 1570 ft
  and rose 0.4 ft. The test is on a perfectly flat floor. A gentle grade on the lane would shift the original's terminal
  speed by a fraction of a ft/s; I did not measure the lane's slope under the whole run.
- **The capture has a short bump.** It is airborne on 8 of 480 rows (around row 195, in 5th gear). The terminal window
  (the last 30 rows) is all on the ground.
- **The original's speed was still rising** by about 0.1 ft/s per second at the end. Its true asymptote may be a few
  tenths above 119.86.
- **The teleport is not a game action.** The lane state was made by shifting every copy of the position I could find by
  one delta, while paused. A copy I missed would be wrong for at least one frame. The bike settled and rode normally
  afterwards, but nothing proves the shifted state matches one the game would reach on its own.
- **The 1.8% that is left is not attributed.** It may be drag, the tire stand-in (`wheel_surface_drag` and the tire
  forces in ride.rs are the port's own, not ported) or the gearing. The test tolerates 5%, as the brief asked. No
  port-side number was changed to reach it.
- **The launch is not the original's.** The original reaches 28.8 ft/s at 1 s and shifts into 2nd at 57.8. That is P20's
  gap, and I did not compare it here.
- **The wreck's chassis** keeps `MAX_SPEED` 1000. I did not trace whether the wreck body is built through the same
  constructor (0x001dd580 is its other caller and may be it). Its comment is unchanged.
- **The angular windows** (25/YawRateLimit/25, angular limit 1875) are not wired. That is P27, and turning is P24's task.
- **`+0x218` and `+0x21c`** (933156, 250000) are not identified.
- **The new hash vectors were computed on x86-64 only.** Cross-architecture runs need an upstream toolchain, which this
  machine does not have.
- **The wheel mounts** in the test are the model's nodes, as `tests/replay.rs` reads them. They are not the shell's
  `world` arithmetic, which differs in the last bit of the front z.

## Next

The task is done. The follow-ups are filed:
- P27: the angular windows.
- P28: the `bike_vel` = 0.8 × chassis factor in P15's constants.
- HUMAN.md: the uinput ACL.
