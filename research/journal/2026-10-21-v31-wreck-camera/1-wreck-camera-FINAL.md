# 1 — wreck camera (FINAL)

Task V31. 2026-10-21.

## What I did

- The user's report (2026-10-20) was that after a wreck the camera follows the bike, not the rider.
  The original has a second camera for wrecks, `MXCameraWreck` over `SimpleWreckCam`; it is not the
  follow camera with a new target. It is now ported as `chase::WreckCamera` in
  `crates/openmx/src/chase.rs`.
  - `chase::wreck_targets` gives it the rider's body and the bike. The rider's body is the ragdoll's
    `BACK` part (`[Part4]`) node while a ragdoll is down, otherwise the crash's rider point.
  - It is per player: each `Rig` in `main.rs` has its own `wreck_camera`.
- `main.rs`, per seat each frame:
  - `switch` picks which camera has the screen: the wreck camera while `rider.crash` is set on a
    bike, the follow camera otherwise.
  - The follow camera keeps running underneath. While the wreck camera is on, its `update` view
    replaces the follow camera's.
  - Back on, the follow camera gets `snap = true` and is behind the bike that same frame.
- Tests in `crates/openmx/tests/wreck_camera.rs` (8 pass through `./kit check -- -p openmx --test
  wreck_camera`):
  - `the_wreck_camera_frames_the_rider_at_the_original_distance`: the target is the rider's body,
    not the bike. A lone target settles at the decompilation's 16.89 ft, which is a 19.5 ft spread
    across 80 − 20 = 60°. The look-at is on the rider and the eye is 8° up, side-on.
  - `a_near_bike_widens_the_shot`: the weighted point and the distance that fits a 30 ft spread.
  - `back_on_the_follow_camera_has_the_bike`:
    - the wreck camera starts from the follow camera's view;
    - back on, the follow camera snaps to the bike;
    - a cannonball gets the shorter lags.
  - `each_player_has_their_own_wreck_camera`.
  - `the_field_of_view_starts_from_the_screens`: the base fov is the follow camera's at the switch,
    and the shot never goes past 82.
- Prior art: opentennis `crates/hst-play/src/play/replay_camera.rs` films skeleton points (Spine2,
  Head, Pelvis) rather than the root, and switches between the match camera and scripted cameras.
  - What I took: the idea of aiming at a body node and not the body's origin (the original does the
    same, with its upper-body point), and keeping the match camera running while another camera
    has the screen.
  - No code was taken; its blends do not match this original, which hands back with a snap.

## What the original does

All addresses are in `research/oracle.txt` under V31.

- **The switch.** `./kit fn 1ffcb0` is the screen's wreck notice.
  - The eye starts from the viewport's eye, with a 0.28 s lag (0.2 for reason 13, the cannonball).
  - The look-at starts from the follow camera's look lags: 0.3 / 0.3×0.75 / 0.3 (cannonball 0.23 /
    0.23×0.75×0.9 / 0.23). The 0.75 is the camera's `+0xa0`; it is 0.25 at 16:9.
  - The set-active slot `./kit fn 31ca90` sets the base fov `+0x9c` to the viewport's fov
    (`+0x7c → +0x170`), i.e. the follow camera's at that moment. That is why screen A's base was 85
    live (its follow preset) while the constructor writes 80.
  - The activation `./kit fn 31cda0`:
    - distance and direction come from eye − targets' centroid;
    - the distance offset is distance − 13, clamped;
    - the spread and the fov offset are zeroed.
- **The targets** (`./kit fn 31c530`, `./kit fn 31cf60`).
  - Slot 0 is the rider body object's CameraInterface (obj+0xa80). Its point (`./kit fn 319050`)
    sits on the upper body. Live, it was nearest the `[Part4]` node: 0.05–0.15 ft at rest, about
    0.7 ft in flight.
  - Slot 1 is the rider's bike camera block, with flag &8: its velocity stays the one it had at the
    wreck entry.
  - point = (T0 + T0 + (T1−T0)·(2/3)·max(0, 1 − d/45)) / active. The 45 ft radius is 25 at 16:9.
  - The velocity is the average of the two. The rider's is its point differenced; vertical × 0.7
    under 20 ft/s.
- **The frame** (`./kit fn 31c670`, 30 Hz; the lags are handed 1/30 s).
  - Spread: the targets' distance apart, or 19.5 (1.5 × 13) with one target.
  - Fov: offset walks by a tenth of the spread's growth (`./kit fn 31d480`). Clamped to [20, 82]
    with a slip: past 82 the offset is set to 62, the clamp's span (`./kit fn 31dcd0`). This is the
    live 62 / −3 alternation.
  - Distance: eases 0.05 a frame toward the distance that fits the spread in fov − 20°, clamped to
    [13, 100] (`./kit fn 31e2d0`).
  - Aim: pitch eases 0.05 a frame to 8° (0.1396), and yaw to π/2 once the spread is over 3 ft.
  - **Direction is not eased.** It is set to the new aim + 0.1 × (new aim − old aim), normalized. I
    first read it as a 0.1 ease. The replay below caught it: the logged direction's y is exactly
    sin 8° within frames, and it flips the same frame the bike leaves the radius.
  - Look-at: point + 0.2 s of velocity while moving, held inside a 1 ft deadband
    (`./kit fn 31de70`).
  - The eye floor is 5 ft, or three near planes.
- **Back on:** the follow camera takes the screen with a snap, and the target list is emptied. Yaw
  and pitch persist into the next wreck: yaw was already π/2 at the start of the fourth live wreck.

## How it was proved

- Function reading as above, plus PINE logs of the original in PCSX2 (KIT_PCSX2=4). The wrecks were
  lean-back wrecks driven by a pad script. Every frame the wreck camera had screen A, I logged both
  targets, the viewport eye, the look and eye lags, distance, yaw, pitch, fov, spread, both offsets
  and the direction.
- The tuning (`+0xa0` 0.75, `+0x114` 45, the param block, the floor 5, the ceiling FLT_MAX) was
  read live. It matches first_recording's RAM image except for the base fov (80 there, before any
  wreck).
- **Replay**: 106 logged frames of one wreck, fed frame by frame through `WreckCamera::update`. The
  port was seeded from the first row (eye, look lags, yaw, pitch, fov lag, base 85) and given each
  row's targets. The test was temporary and is not committed; the csv is context data.
  - Look-at: worst 0.37 ft.
  - Direction: equal to 3 decimals on every row after the first.
  - Fov: within 0.3° (79.13 → 81.8 → 80.5 as the original).
  - Eye: within 0.25 ft for the first 40 frames. After that only y departs: the original's eye is
    held at about 230.5, a ground floor, and the replay had no ground. The original's y is never
    below the port's (min −0.01).
  - Horizontal eye error: mean 0.77 ft, worst 2.4 ft. The worst is the distance running up to
    2.5 ft long while the spread grows (see below).

## Not verified / not 1:1

- **Spread grows faster than the original's.** The port takes |T1 − T0|. The original's logged
  spread lags that while the bike flies off (6.2 vs 8.2 ft) and catches up once they are far apart.
  As a result the port's distance runs up to 2.5 ft long mid-wreck. The cause was not found: it may
  be a T1 point read a frame earlier, or reads torn by the logger.
- **The writer of the rider point's off-bike value** (request+0x1e0) was not found. The port uses
  the ragdoll's `[Part4]` node, which was within 0.05–0.7 ft of the original's point live.
- **The eye floor and the collision push are only partly ported.** The floor is ported (max(5,
  3 near planes) over the ground). The replay could not check it because it has no track ground.
  The collision push through `+0x10c` is not ported.
- **Not ported:**
  - the transition-timer path of the set-active slot (`+0x2b4`/`+0x2bc`), not seen used in a wreck;
  - the ceiling `+0x38c` (200 when the level's `+0x1c` > 1500).
- **16:9 values** (radius 25, look ratio 0.25) come from the decompilation only; I did not check
  them live.
- **Not the ragdoll task's exact wrecks.** The live wrecks were my own lean-back wrecks from a pad
  script, not the first_recording wrecks the ragdoll tasks used.
- **The heading before any fast motion** falls back to +z, not the primary target's forward row.
- **Frame rate.**
  - The 0.05 aim ease is rescaled from 30 Hz to the shell's dt.
  - The direction's 0.1 overshoot is applied per shell frame, so at 60 Hz it overshoots about half
    as far. It is a tenth of a frame's turn either way.
- **tan** comes from std, not the original's curve table, in the fitting distance.
- **The first wreck's yaw and pitch** start at 0, as the original's did at its first wreck. Yaw and
  pitch persist across wrecks as in the original. The fov lag likewise starts at the constructor's
  80 and is not reseeded at the switch (the log shows 79.0 eased up, consistent with this).
- **The follow camera keeps updating** underneath the wreck camera. I did not check whether the original runs it
  while the wreck camera has the screen. It snaps on handback, so this is not visible.

## Next

Nothing for V31. A follow-up could look into the spread lag (log T1's point alongside the spread
from inside the frame, at the write).
