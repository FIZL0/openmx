# 1 — trick ragdoll transitions (FINAL)

Task S257. 2026-11-02.

## What I did

- **Captured recording2 played back, twice.**
  - `capture/bones_playback.py` is S74's `bones_watch.py` format, with no state load and no pad. It waits
    for `frame_counter` in FIRST..FIRST+26 with the human bike found, then logs N ticks.
  - It reads these regions:
    - the bike node;
    - the body words `body+0x2a4` (riding, repose, placed), `+0x47c` (mode), `+0x680..` (vehicle
      animation: from_table, holding, ramp) and `+0x8dc..` (state running / asked);
    - the request `+0x488` (0x1b0 bytes);
    - the 22 joints and nodes;
    - for the second run, every part record (`*(body+0x2f4) + 0xb0·i`), its body's start (`+0x30`) and
      its body (`+0x50..`).
  - `capture/play.sh` is V61's AT-SPI playback with this poller. It plays a *copy* of the `.p2m2` and its
    `_SaveState.p2s` from a scratch dir.
  - The captures are git-ignored, in `context/s257/`. `recording2.txt` and `recording2-parts.txt` each
    hold 1700 ticks, frames 2374..4073, torn 0, gaps 0. The two runs agree (node 3 differs by 0.0
    between them).
  - The tape's md5 (`1e5091236f40915ad1e5942bce20287d`) did not change.
  - PCSX2 copy 1, `KIT_PCSX2=1 ./kit pcsx2 with -- bash …/play.sh PID context/recordings/recording2.p2m2
    2374 1700 context/s257/recording2.txt`.
- **Port, the trick exit** (`crates/game-sim/src/ragdoll_riding.rs`, `step_under` / `Pose`).
  - A posed tick now keeps the drawn matrices from before it, in `Riding::was`. That field is only read
    while placed by the wreck hand-over, so nothing on the wire changes.
  - The riding entry out of a posed tick poses each bone at the clip's node as the last posed tick drew it
    (`Riding::drawn`). It moves the bone by that node's move over that tick (against `was`), with no still
    pose first.
  - The put-back entry (S201) is unchanged: `Pose::still_first`.
- **Port, the tick the words are read** (`crates/game-sim/src/rider.rs` `frame_bumped`,
  `Riding::ride`, new `Riding::off_the_table`).
  - The rider's riding tick read `body.machine.mode == Posed` at the top of the frame. The shell runs
    `Rider::frame` first and `Body::ride` (the machine's mode, then the vehicle animation's step) after
    it. So that mode was chosen from `from_table`/`holding` as they stood *before* last frame's step.
  - The ragdoll therefore let go a tick after the original's did, and took the bones back a tick late.
  - It now reads `body.veh.from_table` and `body.veh.holding` as last frame's step left them, into
    `Actor::posed` / `Actor::latched`. `posed_off_the_table` is kept; it is unused by the rider now.
- **Tests.**
  - `crates/game-sim/tests/trick_ragdoll.rs` (new) steps the port's `Riding` through all of the capture,
    on the original's requests and skeleton.
  - `crates/game-render/tests/body.rs` `a_gesture_unplaces_the_riding_ragdoll_on_the_originals_tick`
    (new) runs the shell's own wiring (`Rider::frame` then `Body::ride`) through a gesture.

## What the original does

Body actor = the type-4 record less 0x400. Its mode word is `+0x47c`: 0 held, 1 riding, 2 wrecked, 3
posed. Placed is `+0x2ac`. The vehicle animation is the body's `+0x680` from_table, `+0x684` holding,
`+0x68c` the cross-fade ramp, `+0x8f0` the state running and `+0x8f4` the state asked.

- **A trick does not run in mode 1, and nothing pulls the bones toward the stunt clip.**
  - The mode chain (V61) sends the body to mode 3 when from_table (as the previous tick's animation left
    it) or holding is set. Holding is set by the body mode for a backflip before it tests.
  - The posed entry clears placed and runs `rider_body_look_reset` (`./kit fn 0x0024d5f0`). The ragdoll
    is neither stepped nor drawn, and the riding pose spring does not act at any strength.
  - The 22 nodes are the clip's as the animation wrote them, cross-fade included: the posed nodes are
    within 0.0054 ft of the skeleton composed from the captured joints, on all 368 posed ticks.
- **Fade in.**
  - On the tick a gesture's table goes up (from_table 1, state 0..23, ramp starts at 0.867), the body is
    still mode 1 and placed. That tick's riding step pulls toward the skeleton mid cross-fade into the
    stunt.
  - Mode 3 starts on the next tick.
  - A backflip (state 38) raises holding through the request's `+0xfc`, so it goes posed on the tick
    itself (2512).
  - The animation's ramp rates, per 30 Hz tick:

    | transition | ramp per tick | per second | duration |
    |---|---|---|---|
    | into a gesture | 0.1333 | 4/s | 0.25 s |
    | backflip in, or out to the AIR row (35) | 0.1083 | 3.25/s | |
    | out to the pose grid (30) | 0.1333 | | |
    | into the wreck (40) | 0.0083 | | |

- **Chain.** The backflip went into gesture 0 (2512..2594: state 38, then 0 at 2543 with from_table up,
  holding dropping at 2550). The body stays mode 3 throughout; a chained trick is one posed run.
- **Fade out.**
  - On the tick from_table drops (the state is now 35 AIR, or 30 pose grid, ramp 0.892), the body is
    still mode 3.
  - The next tick, E, is mode 1. The riding entry `rider_body_ride_enter` (`./kit fn 0x0024c490`) runs
    the moving `rider_ragdoll_pose` (`./kit fn 0x0030f0c0`).
  - Each body's start position is the node as tick E−1 left it: the clip under last tick's bike, as in
    S201's put-back. Its start velocity is `(node − part+0x90)/dt`.
  - Through a posed run, `part+0x90` (the drawn matrix's translation) is the node of the tick before, on
    every posed tick. Something writes it each tick; I did not find what. So the velocity is the node's
    move over E−1, about the bike's velocity. On exit 2595 that is −17.80, −46.49, −36.10 ft/s for part
    0, against (4173.58 − 4174.17)/0.0333 in x.
  - From E on, the spring pulls toward the animation still cross-fading out of the stunt, under last
    tick's bike (S90). The ramp goes 0.783, 0.675, 0.567, 0.458 … at AIR's rate.
- **Bailed / cut short.** Trick 3522..3562 (gesture state 0) never came back.
  - At 3563 the body went from mode 3 straight to mode 2 (state 40, ramp 0.992), with from_table cleared
    on the same tick.
  - The wreck enters from the posed clip. The ragdoll was never placed in between, and mode 2 shows
    placed 1 from its entry on.
  - The put-back followed at 3586 (held 3586, 3587, riding from 3588).

Tricks in the capture:

| frames | states | note |
|---|---|---|
| 2512..2594 | 38, 0, 35 | backflip into gesture 0, held |
| 2651..2697 | 2, 35 | |
| 3300..3387 | 1, 35 | held |
| 3522..3562 | 0 | bailed: Posed → Wrecked at 3563 |
| 3753..3812 | 15, 35 | |
| 3927..3980 | 2, 35 | |

## How it was proved

- **A save-state playback capture compared tick by tick**, `crates/game-sim/tests/trick_ragdoll.rs` on
  `context/s257/recording2.txt` (1700 ticks). Each tick feeds the original's request, dt, skeleton (the
  captured joints composed under last tick's bike) and last row's from_table/holding.
  - The port's mode and placement equal the original's on all **1553** stepped ticks, 368 of them posed.
    The 6 tricks are found.
  - The posed nodes are within **0.0054 ft** of the clip.
  - Entry ticks (5; the backflip has none): worst **0.0994 ft**, worst turn 0.1725, mean 0.029.
  - The first 4 riding ticks after each of the 5 exits (20): worst **0.0276 ft**, worst turn 0.0492, mean
    0.0046. Before the exit fix they were worst 2.13 ft, mean 0.80. With the bones at the clip under the
    moved bike, it was worst 1.45.
  - Plain riding (1126 ticks): worst 0.2075 ft, worst turn 0.3685, mean 0.021.
  - It asserts 6 tricks, 368 posed, clip < 0.01, entry < 0.15 ft / 0.25, exit < 0.05 ft / 0.08, riding
    < 0.3 ft / 0.5, mean < 0.04.
- **The shell's order**, `crates/game-render/tests/body.rs` on the disc: the rider's shortest gesture
  ("cancan") in the air.
  - At every riding or posed tick, the riding ragdoll is posed and unplaced exactly when the previous
    frame's `Body::ride` left from_table or holding set: 80 posed ticks, let go once and taken back once.
  - With the old `machine.mode` wiring put back by hand, it fails at frame 11, the first tick of the
    gesture.
- `./kit check -- -p game-sim --no-fail-fast`: everything passes but `trick::the_humans_tricks`. That one
  is red on C6's freeride capture in this slot before and after this change (memory note). `-p
  game-render`: 285 pass. `-p openmx --no-run` builds.

## Not verified / not 1:1

- The start positions and velocities in `recording2-parts.txt` were not compared bit for bit with the
  port's pose, as S201 did. Only the nodes after the step were compared, within 0.03 ft. The port reads
  its drawn matrices from its own `ragdoll_ride::nodes` of the captured skeleton, not from the part
  records.
- What writes `part+0x90` every posed tick was not found. The port's `Riding::drawn` is written every tick
  anyway, which gives the same value.
- The bailed trick's wreck entry, from mode 3 with nothing placed, is not compared: the test's stretch
  stops at mode 2. The port's `hand_over` gives `None` for an unplaced body, so its wreck enters from the
  skeleton. I did not check whether that is the original's start for the wreck bodies.
- The entry tick's 0.1 ft and the riding 0.2 ft are the riding step's own error (S135's bounds), not the
  transition's.
- The game-render test's holds are the chassis anchors under an identity root, not the drawn chassis'
  root. They do not move the tick the body lets go on, but the bones there are not checked.
- Only recording2's six tricks were seen. No spin trick (state 15 is a gesture slot here), no trick cut
  short by landing rather than a wreck, and no chained gesture-to-gesture without a backflip.

## Next

Nothing waiting for this task. Follow-ups are filed in the plan: the wreck from a posed body, and the
`part+0x90` writer.
