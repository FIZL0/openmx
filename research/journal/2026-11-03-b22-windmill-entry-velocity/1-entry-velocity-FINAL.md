# 1 — the entry's velocity off the windmill (FINAL)

Task B22. 2026-11-03.

## The capture

s3's `context/capture/b15` is gone, so it was recorded again on PCSX2 copy 2:
- B15's `midback-2734.p2s` was loaded;
- the run was `research/r14_windmill_bones.py PID SLOT <south3 point> 8 south3-bones.csv bones ride=0`
  under `./kit pcsx2 with --`.

It is 225 rows, with `-before.ram` and `-after.ram`, and it lives in s2's `context/capture/b15/`
(git-ignored). The riding word drops on row 5 and the reason is 4, as before.

PCSX2 needed the following:
- `QT_ACCESSIBILITY=1` and `QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1`, or AT-SPI finds no app;
- the slot folder is `savestates/`;
- `./kit pcsx2` asks `agent-runner home slots`, which needs agent-runner 0.13.1+, but 0.12.0 is
  installed. A PATH shim answered it (filed as T).

## What parted the bones

Per tick, the drawn history (`Riding::hand_over().1`) was logged in `b19_ride`. On the verdict's tick
(port tick 3) it was `None`: `riding_tick` had dropped the seeded body and built a `Riding::new`.
- `Riding::strayed(at)` is the port's stand-in (S110) for a bike teleported or reset away from its
  bones, with STRAY = 10 ft.
- `at` was `self.ride.position()`, the bike after this tick's integration.
- The windmill's throw moves the bike 10.8 ft in that one tick, so the bones (still at the top of
  the tick) were "strayed".
- With no history, `Down::enter_from` gives each bone `crash.bike.body.velocity`, the (8, 16, 133)
  of the brief.

On R14's south-bones the throw moves the bike less than 10 ft, which is why that entry held.

Fix (`rider.rs riding_tick`): `at = before.0[3]`, the bike's translation at the top of the tick. The
bones hang under that frame, so a bike's own travel in a tick never counts as a stray, while a
teleport or reset between ticks still does. The original has no such rule; it never rebuilds its
body. The brief's three suspects (the request's frame, the bike box, the hold release) were not
needed.

## The legs: a test seed, not the port

With the body kept, the entry's mean was 0.5 ft, but the legs were up to 3 ft off. The port's
`body.veh.state` defaults to clip 30, and the seeded actor's clip is 35. The first tick's clip
change made the look-around feet switch drop foot hold 0 (bone 17). No shell runs the vehicle
animation in the replay, so `b19_ride` now seeds `sim.rider.body.veh.state = riding.actor.clip`.
This changes only the test. R14's numbers are unchanged.

## Numbers (B15 south3-bones, seeded; mean / worst ft off the csv)

| tick | HEAD | B22 |
|---|---|---|
| 1–3 (riding) | ≤ 1.7 worst | 0.25, 0.56, 0.87 worst |
| 4 (entry row 5) | 3.4 worst | 0.49 / 0.87 |
| 11 | parting at ~2.2 ft a tick | 1.39 / 1.61 |
| entry main bone, ft/s | (8, 16, 133) | (40.4, −37.3, 121.9); original's (41.4, −38.6, 121.6) |

After touchdown (~tick 12) the contacts part as on R14. A residual ~0.12 ft a tick of drift remains
on both captures, with x velocity 1–3 ft/s low. That is filed as a low R.

## Checked

- `tests/ragdoll.rs a_thrown_bike_keeps_its_riding_body`:
  - a one-tick 600 ft/s throw under a raised speed limit (20 ft of travel) keeps the history;
  - a 500 ft teleport still starts afresh;
  - it is red under the old rule ("24.73 ft").
- `tests/replay.rs the_windmill_strikes_the_ports_bones_left_at_its_face`:
  - worst < 1.0 ft riding and < 2.0 ft through tick 11;
  - the entry's main bone within 2.5 ft/s of the original's.
- `the_bones_enter_the_wreck_a_tick_after_the_verdict` (R14) is unchanged and green.
- `./kit check -- -p game-sim --no-fail-fast`: 1163 passed.
- `./kit check -- --workspace --exclude game-sim --no-fail-fast`: 3306 passed.
