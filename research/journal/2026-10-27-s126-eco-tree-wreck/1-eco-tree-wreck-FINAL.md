# 1 — eco tree wreck (FINAL)

Task S126. 2026-10-27.

## What I did

- **Found the routing already in place.** S138 (landed after S100 filed this) routes the riding
  bike's eleven body rays through the world's props *and trees*: `rider_contact::scenery` calls
  `flight_search::Scene::wooded`, and the bump's resolve loads the chassis shock and so g_peak.
  S157 (`tests/two_p_world.rs`) proved that resolve bit for bit on the 2p ride's trunk contacts. S100's
  port ride wrecked nowhere because its `Rider::new` carried no bike collision files. Without them
  `scenery` is never called, and the ride falls back on the frame's velocity probe, which loads nothing.
  So no code in `ride.rs` changed, and no code needed to.
- `crates/game-sim/tests/eco.rs`:
  - New helper `the_body_rays()`: `Bike\Col\bikem.col` and `bikef.col` out of `free09_ss_2.res`,
    read the way roof_landing.rs reads them.
  - `a_free09_tree_against_the_original`: the original's half is unchanged. The port's half now
    carries the body rays and rides twice: from the capture's first place at rest, and from ten
    ticks out at the original's place and speed there. For each ride it records the tick the rays
    first find the trunk. It asserts wreck 1 on that same tick, with g_peak.z past the 125's
    FrontWreckSpeedLimit (`phy.air.front_wreck_speed_limit`).
  - `riding_into_a_free09_tree_stops_the_bike_at_its_trunk`: carries the rays and expects wreck 1 on
    the tick the trunk stops the bike, and no wreck on the open floor. Its "stop" is now "a flat-out
    bike loses over 15 ft/s in a tick". The old test was "falls under 10 ft/s", but a wreck freezes
    the chassis at about 22, as the original's freezes at its last values.
- `crates/game-sim/tests/replay.rs`: new `a_free09_trunk_wrecks_as_the_original_does`. It replays
  S100's own ride in the port from the original's state (`context/capture/s100/tree.ee.bin`, the
  `eeMemory.bin` of S100's `tree.p2m2_SaveState.p2s`). It uses `SaveState::seed_ride` for the whole
  bike, free09's ground, props and every trunk of `free09.esb` (`Trees::on_ground` under `Wooded`),
  the body rays, and the capture's pad. This is S138's walls test pattern.
- `crates/game-sim/src/eco.rs`: the module doc no longer says the port has no body rays.
- `research/oracle.txt`: an S126 note under S100.
- Filed **S184**: once P154 makes the launch the original's, take `S100_EARLY` to 0 and compare the
  hit tick's numbers row by row.
- Copied S100's captures and states from s2 into this slot's `context/` (git-ignored).

opentennis: nothing there for this (no bike collision).

## What the port does now

| ride | rays find the trunk | in | after | g_peak | wreck |
|---|---|---|---|---|---|
| original (S100) | tick 61, 2 contacts, 3.13 ft off | 58.16 | (nowreck) 34.8 | (-9.19, 14.22, -28.94) | 1 at 2978 |
| port from rest, Flat | tick 50, 1 contact, 4.81 ft off | 63.81 | 22.05 | (-13.34, -3.24, -41.72) | 1, same tick |
| port 10 ticks out at the original's speed | tick 10, 1 contact, 5.83 ft off | 51.75 | 26.50 | (-8.75, 3.74, -27.15) | 1, same tick |
| port from the original's state, free09 | counter 2974 | 57.38 | 22.50 | (-11.52, -4.26, -35.82) | 1 at 2974 |

The depth of the hit depends on the tick's phase. The front ray ends 3.6 ft ahead of the bike's
origin, so a bike 4.8 ft from the axis reaches the facet with only that ray. The original stepped
from 5.07 ft (just short) to 3.13 ft, deep enough for two rays. While trying ride starts, a port ride
started 5.05 ft out found 2 contacts at 4.14 ft, as the original's did.

## How it was proved

- `./kit check -- -p game-sim --test eco`: 8 pass. `--test replay a_free09_trunk`: passes.
  Without the body rays the same rides print g_peak 0 and no wreck (S100's output and the first run here).
- The replay asserts four things: reason 1 as the original; g_peak.z past the front limit; the
  centre of mass 2.5–5 ft off plant 347's axis (4.34; the original's 3.49); and a ratchet
  `S100_EARLY = 4` on how many ticks before the original's the port wrecks.

## Not verified / not 1:1

- **The wreck tick is 4 early from the original's state** (2974 against 2978), held as a ratchet,
  not matched. The cause is the launch (P154): the seeded 125 does 5.10 ft/s at 2920 where the
  original does 1.84. S184 takes the ratchet to 0 after P154.
- **g_peak does not match the original's.** The port's (-11.5, -4.3, -35.8) against (-9.2, 14.2,
  -28.9): y even has the other sign. The bike meets the trunk at another phase, one ray deep rather
  than two, so the resolve's inputs differ. The resolve itself is S157's, bit for bit on trunks; it
  was not re-proved on this hit.
- **The post-hit velocity is not compared** (port 22.5 against the original's ~28 frozen; nowreck
  34.8). Same cause.
- **The wrecks-off ride is not ported or compared.** The original rears up, comes round and rides
  back along −z. The port has no wrecks-off switch on this path in the test, and the hit's phase
  differs anyway. This is folded into S184.
- **The eco.rs rides are on a Flat floor at the plant's foot**, from `Rider::new`, not the
  original's state. The "at speed" ride sets only the chassis velocity and the wheels' spin. The
  engine, gear and suspension are fresh, so it slows 58→52 over ten ticks.
- **The put-back after the wreck** is not compared (as in S100).
- **Only one trunk, head-on, the 125** (as in S100).
- **The 2-tick pad lag at the state's start** uses the tape's first (idle) row for counters 2916/2917.
  The tape holds Cross from row 1. This was not checked against the original's controls for those two
  ticks.
