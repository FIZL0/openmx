# 1 — c7 biplane capture (FINAL)

Task C7. 2026-10-20.

## What I did

- **The answer first.** `context/recordings/biplane.p2m2` (ryha, 3,437 fields, about 57.3 s, PCSX2
  2.9.127, Free06) starts with the human **already in the plane, standing on the ground**:
  - body y 138.1, speed 0.6, on_ground 1, throttle 0.01
  - the plane is entry 0 of the GameManager's actor list in the save state, with ctl_src 1 and +0x334 = 1 (flown)
  
  **Nothing switches from the 250.** On all 1,718 rows of the capture:
  - the plane is the flown actor (plane_flown 1, plane_ctl_src 1)
  - the human's rider (entry 3) keeps player 0 and its wreck_reason 14
  
  The plane takes off twice, from the same spot, because a crash puts it back there. **There is no switch tick.**
- I wrote `watch/biplane.toml`, a spec of its own. It shares acceptance.toml's clock, pad and rng blocks (frame_counter, dt,
  rng_state, the 16 held controls, both pad halves with their stamps), plus the plane:
  - **The plane:** `@game_manager -> 0x34 -> 0x34 -> 0x0`, entry 0.
    - Transform: the drawn mat4 +0x600, body position, velocity, quaternion and angular velocity (body
      +0x160 -> +0x60/+0x70/+0x170/+0x1b0), euler +0x5f0, local velocity +0x5c0, local rate +0x5b0, speed,
      alpha, beta, on_ground.
    - **The eight controls as the game reads them:** the player record's sticks (+0x324/+0x328), its control
      table (+0x334, 16 entries × 0x18, value at +4), and the commands the input step writes from them:
      throttle up +0x338, throttle down +0x33c, elevator +0x340 (−1 climb, +1 dive), aileron +0x344 (+1
      bank left, −1 right), rudder +0x348 (−1 left, +1 right).
    - **What those commands become:** throttle +0x34c, pitch +0x354, elevator deflection +0x358, aileron +0x35c,
      rudder +0x360, nose wheel +0x364, the pitch integrator +0x584.
    - **The prop:** the engine's (+0x400) power, rpm +0x60, thrust, the drawn prop's speed +0x44, its angle +0x48,
      and the sound rpm +0x70.
    - **The crash state:** +0x3b0, +0x3b4 and the clock +0x3bc.
  - **The rider (entry 3) and the bike (entry 7):** their identities, the rider's wreck_reason, and the bike's
    velocity and transform, so a reader can see that nothing moves the human off the plane.
  - Every field was first checked against the save state's RAM image (`./kit state extract`). The values
    were the ones the decompilation predicts: speed = |body velocity|, prop rpm 500 + 2000·throttle, and
    MinWreckSpeed 73.33.
- **Playback.** I played a **copy** back on the original in PCSX2 copy 1 with C6's AT-SPI recipe (`playback/play_capture.sh`) and
  captured it to `context/capture/acceptance/biplane.csv` (+ `.json`).
  - **Size:** 1,718 rows × 143 columns, ticks (frame_counter) 10971..12688 in steps of 1, vsync 27244..30678, and not one empty cell.
  - **Per-row checks:** dt is 0.03333 on every row, and plane_type 1 holds on every row.
  - **Checksum:** md5 792d20b8b62fe7f1b67a7bf7a3a0c055. It is git-ignored, like every capture.
  - **The tape is unchanged:** the worktree's recording is chmod 444, and its md5 is still 8a0fd1f5f29d08e9f1ef6e91a87dfba1 after both runs.
  
  It differs from C6's recipe in three ways:
  1. **The spec** is its own.
  2. **`start`** waits for `plane_type == 1` instead of the rider.
  3. **The clock:** the recorder wakes on the pad stamp `pad0_stamp_a` (0x47a9d8), not frame_counter. S32 showed that a
     frame_counter wake reads some rows mid-tick. The script sets `vsync_addr` in a temporary copy of kit.toml passed
     as `KIT_CONFIG`; the worktree's kit.toml is not touched.
- **Addresses.** I found every address in the decompilation myself and wrote them to `research/oracle.txt` (the C7 block) for
  R8.
- **Scripts in `playback/`.** `check.py` checks a capture's integrity and its input against the tape. `index.py` prints
  the index below. `compare.py` compares two captures cell by cell.

- **One test change.** P22's `a_flight_falls_and_carries_like_the_originals` (crates/game-sim/tests/air.rs)
  fits bike flights in *every* csv in `context/capture/acceptance/`, so the biplane capture failed it ("biplane.csv:
  only 0 ballistic ticks"): the bike stands parked through the whole recording. The test now skips captures
  with a `plane_type` column. Every other capture it read before is still read, and it passes on the
  three bike captures in this worktree.
- **This worktree only:** the Oct 17 `acceptance-ride.csv` and `acceptance-start.csv` were older than the newer readers
  and failed the same test with "only 198 ballistic ticks". I moved them to `context/capture/acceptance/old/`, the fix
  noted for old captures. That is context, not committed.

## What the original does

- **The controls** (`./kit fn 0x00351e20`, reached through vtable +0x20 from the player record's input,
  `./kit fn 0x0025e910`). The commands are looked up through the pad's binding map. In this state, pad 0 binds them like this:

  | Control | Binding | Value |
  |---|---|---|
  | throttle up | Cross (command 0xe) | |
  | throttle down | Square (0xf) | |
  | elevator | Up (2) | +1 (dive) |
  | | Down (3) | −1 (climb) |
  | | otherwise | −sign(y)·\|y\|³ of the left stick |
  | aileron | Left (0) | +1 |
  | | Right (1) | −1 |
  | | otherwise | −sign(x)·\|x\|³ |
  | rudder | L1 (0x14) | −1 |
  | | R1 (0x15) | +1 |

- **The joystick step** (vtable +0x19c, at 0x00352160; Ghidra made no function there, so I read the disassembly) rate-limits these:
  - **Pitch:** into a pitch demand, scaled down with roll.
  - **Aileron and rudder:** into their deflections times MaxAileron and MaxRudder.
  - **Throttle:** steps by ThrottleIncrement·dt, clamped to [ReverseThrottle, 1].
  
  The autopilot (vtable +0x1a0, 0x00352460) turns the pitch demand into the elevator deflection with a pitch-rate loop.
- **The engine** (`./kit fn 0x003501b0`) has two speeds:
  - **Prop rpm:** lerp(500, 2500) by throttle. The capture holds this on every flying row (523 at 0.01, 2500 at 1.0).
  - **The drawn prop:** turns at a constant +0x44 = 217.25 × 0.1047198 rad/s, whatever the throttle.
  
  So "the prop's speed" is two columns: `prop_rpm`, the engine's, and `prop_turn`/`prop_angle`, the drawing's.
- **The crash state** (`./kit fn 0x00353180`):
  - **0, flying.**
  - **1, crashed:** a contact above MinWreckSpeed (73.33), a surface that is not ignored, or a contact while upside down.
  - **0xd, thrown:** out of the area; the step sends the plane back toward the centre with random spins.
  
  State 1 ends after WreckTime (5 s), or earlier through the **L1 + R1 chord** (`./kit fn 0x00355df0`, commands 0x4c/0x4d, bound to L1 and R1),
  which works on the ground or crashed. Either way, `./kit fn 0x003536e0` puts the plane back at the start: (2304.0, 138.6, 2281.0) both times.
- **On every crash row the throttle reads 0.00 on the first tick, then creeps by 0.011 a tick.** Meanwhile prop_rpm stays at 2500
  until the plane is back. So the engine is not stepped while crashed (observed, see below).

## The index

Rows are capture rows; tick = frame_counter; seconds from the first row (30 ticks a second).
`python3 playback/index.py context/capture/acceptance/biplane.csv` prints all of it.

| When | What |
|---|---|
| row 0 (tick 10971, 0.0 s) | On the ground, still, throttle 0.01, prop rpm 523 |
| row 24 (10995, 0.8 s) | Cross: throttle up; held 1,391 rows in 3 runs over the take |
| row 109 (11080, 3.6 s) | Throttle 1.0, prop rpm 2500 |
| row 111 (11082, 3.7 s) | First elevator −1 (climb, Down / stick back) |
| **row 140 (11111, 4.7 s)** | **Take-off**: on_ground 1 → 0 at 63.7 ft/s, throttle 1, elevator −1 |
| rows 158–240 (5.3–8.0 s) | Climb, y 141 → 223, nose up to +0.70 |
| rows 181–267 (6.0–8.9 s) | Bank left, roll down to −1.62 (on its side), at row 228 |
| rows 263 / 297 | First rudder right (R1) and left (L1) |
| rows 279–360 (9.3–12.0 s) | Climb, y 227 → 309 |
| rows 298–355 (9.9–11.8 s) | Bank right, roll up to +0.88 |
| rows 373–480 (12.4–16.0 s) | Dive, y 309 → 223, speed 85 → 115 |
| rows 416–473 (13.9–15.8 s) | Bank left, roll down to −0.91 |
| rows 487–597 (16.2–19.9 s) | Steep climb, y 223 → 401, speed 111 → 49; nose +1.45 at row 533, vy +69 at row 518 |
| row 606 (20.2 s) | Highest point, y 402.6 |
| rows 589–663 (19.6–22.1 s) | Bank right, roll +1.57 at row 641 (on its side) |
| rows 613–692 (20.4–23.1 s) | Dive, y 401 → 245, nose −1.30 at row 656, vy −82 at row 667, speed up to 101 |
| **row 693 (11664, 23.1 s)** | **Crash 1**: hits the ground (on_ground 1) at 101 ft/s, nose −0.83; crash 0 → 1, throttle → 0 |
| rows 768–769 | The wreck touches down again at y 141 |
| **row 776 (11747, 25.9 s)** | **The human ends crash 1 with L1 + R1** after 2.73 s (rows 776–779 hold both): the plane is back at the start, crash 0 |
| rows 777–905 (25.9–30.2 s) | On the ground again; Cross back on, throttle 1.0 by row 861 |
| **row 906 (11877, 30.2 s)** | **Second take-off** at 76.2 ft/s, throttle 1, elevator −1 |
| rows 919–1015 (30.6–33.8 s) | Climb, y 142 → 240 |
| rows 930–1053 (31.0–35.1 s) | Bank left, roll down to −1.51 |
| rows 1034–1079 (34.5–36.0 s) | Dive, y 240 → 211 |
| rows 1228–1257 (40.9–41.9 s) | Short climb, y 188 → 199 |
| rows 1253–1296 (41.8–43.2 s) | Bank left, roll down to −0.71 |
| row 1388 (12359, 46.3 s) | Square: throttle down, 13 rows (the only use), throttle 1.0 → 0.85, rpm 2197 |
| rows 1431–1492 (47.7–49.7 s) | Shallow dive, y 181 → 151, top speed 137.5 at row 1492 |
| rows 1493–1497 (49.8 s) | A wheel touches at 137.5 ft/s (on_ground 1) **without a crash**; flies on |
| **row 1527 (12498, 50.9 s)** | **Crash 2**, in the air (on_ground 0) at y 141, 137 ft/s, roll −0.65; crash 0 → 1 |
| row 1678 (12649, 55.9 s) | Crash 2 times out at WreckTime (crash_time 4.999 on the last crash row); back at the start, still |
| rows 1679–1717 (to 57.2 s) | On the ground, throttle 0, prop rpm 500, to the end |

**Throws:** none; crash state 0xd never appears.

**Rolls:** none. The roll never passes ±1.62, so the plane goes to knife-edge, never round.

**The switch:** none.

**Control use, rows held** (the command the game wrote, |v| > 0.05):

| Command | Rows | Runs |
|---|---|---|
| throttle up | 1,391 | 3 |
| throttle down | 13 | 1 |
| elevator −1 (climb) | 597 | 17 |
| elevator +1 (dive) | 175 | 13 |
| aileron +1 (left) | 333 | 22 |
| aileron −1 (right) | 301 | 18 |
| rudder +1 (right) | 150 | 3 |
| rudder −1 (left) | 114 | 4 |

All eight controls are used.

**How I named the directions:**
- **Climb and dive are from the motion.** Elevator −1 is what the human holds through both take-offs, and it correlates with a rising euler.x: −0.26 for the command and −0.46 for the pitch demand, against the next tick's change. Rising euler.x is nose up, the sign the climbs carry.
- **Left and right are from the bindings** (d-pad Left → aileron +1, L1 → rudder −1). Aileron +1 drives euler.z down (−0.53), so negative roll is the bank Left makes.

## How it was proved

- **The input is the tape's.** Every row k ≥ 1's pad half (the one with the smaller stamp, bytes 2..7: buttons and four
  axes) equals biplane.p2m2 field 2k−1: 1,717 of 1,717. The next-best offsets match 1,176 (2k) and 1,161 (2k−2).
  This is C6's TickPhase::Second rule. `python3 playback/check.py CAPTURE context/recordings/biplane.p2m2`.
- **It repeats.** A second playback (`context/capture/c7/biplane_run2.csv`, md5 11106ea2…) passes the same tape check, 1,717/1,717.
  `playback/compare.py` against the first run:
  - 94 of 143 columns are identical on every row.
  - **Body, transform, euler, speed and crash:** differ on exactly one row in each run. That row is a torn read: row 150 in run 1 and row 709 in run 2 repeat the previous tick's body
    position while other fields are new. Every other row matches bit for bit, so the game ran the same flight both times.
  - **stick_x/stick_y and the elevator/aileron commands:** differ on 6 rows, 158–179 and 707–709. These are written per field, and the
    recorder caught a different field on those rows.
  - **rng_state** differs on 231 rows: something draws per vsync, not per tick.
- The oracle's numbers were checked against the state's RAM before the run, and against the capture after it:
  - prop_rpm = 500 + 2000·throttle
  - speed = |body_vel|
  - the crash-2 timeout at WreckTime 5.0
  - the reset chord at row 776

## Not verified / not 1:1

- **One torn row per run.** In this capture, row 150 (tick 11121) carries the previous tick's body, transform and euler while its
  sticks and commands are new. The pad-stamp clock did not remove tearing: run 2 tore at row 709 instead.
  A consumer should treat a row whose body_pos equals the previous row's while speed > 1 as torn (`check.py`
  counts them: 1).
- **The stick and command columns are read at either field of a tick.** They differ between runs on 6 rows, so they are not a
  per-tick truth on every row. The tape check (buttons + axes vs field 2k−1) holds on all rows in both runs.
- **rng_state differs between runs** on 231 rows; it is not a per-tick value here.
- **The joystick and autopilot are read from disassembly only.** Ghidra made no function at 0x00352160 or
  0x00352460. Their gains (the roll-scaled pitch, the integrator) are not checked against the capture
  beyond "pitch demand and elevator move the way the commands say".
- **The engine not stepping while crashed** (throttle reads 0 then +0.011 a tick, rpm frozen at 2500) is observed
  only. I did not find where it is skipped.
- **The second crash's cause** (in the air, on_ground 0) is not identified: an object, or a surface that is not ignored. The
  spec has no contact or surface field. The wheel touch at rows 1493–1497 at 137 ft/s, above MinWreckSpeed, did
  **not** crash, which my reading of the crash test (any contact above MinWreckSpeed) does not explain. Something
  in that condition is not as I read it.
- **"+0xe0c"** (the chord's once-only latch) and the state's other fields are not in the spec.
- **The actor list order** (plane 0, rider 3, bike 7) is measured on this state only. The spec's chains are fixed indices, and
  `start` checks plane_type, but the rider and bike entries are not checked on every row beyond their type columns.
- **The binding map** (which button is which command) is read from this state's pad-0 map, not from a settings screen.
  The HUD strings were not matched to commands on screen.
- **Euler sign conventions:** "negative roll = left" comes from the binding of Left; nobody watched the playback.
- **Only on x86-64 PCSX2 2.9.127** (the recording's version) and copy 1's settings.
- **The tape's last field (3437)** has no row; the capture ends with the budget at tick 12688.

## Next

Nothing; C7 is done. R8 can take the oracle block and this capture.
