# 1 — riding ragdoll online (PART)

Task P133. 2026-11-21.

## What I did

- Online seats are now drawn from the riding ragdoll, as offline seats are since S90.
- Picked the task's second option, its "draw-only shadow": a **drawn riding ragdoll** beside the session
  (`crates/openmx/src/net/riding.rs`, `DrawnRiding`; not called a shadow in code, which is the renderer's
  `game_render::shadow`), not a step inside the session's ticks. Why:
  - The riding ragdoll's inputs are the renderer's: the grips of the drawn chassis (`Rider::holds`, off the
    shell's chassis clip blend) and the body's skeleton (`Body::ragdoll_posed`). Neither is in the session's
    state, so two machines (or one machine over a rollback) can draw different grips.
  - `Riding` is in the rider's state hash and its wire codec. Stepping it in the session off those inputs
    would desync machines; making it safe means moving the chassis animation into the rolled-back state,
    which is a much bigger change.
  - So the session's riders still get no holds and run no riding ragdoll, exactly as before: their state,
    hash and wreck entry are untouched.
- The drawn riding ragdoll, per shell seat (main.rs, right after `Online::show` copies the session seat in):
  - steps once each time the session's tick count moves on, `Online::dt` long;
  - on the seat as shown (its last tick run, a prediction where pads are still coming);
  - held to the grips of the bike drawn last frame (captured after the chassis rides, where offline sets
    `Rider::holds`), pulled to the skeleton the body was just posed in;
  - reads the bike's transform, clip-table words and clip from the previous shown tick, as `Rider::frame_posed`
    reads them off the top of the tick;
  - starts afresh when the bike moved past `STRAY` (a re-run, a put-back), and is dropped while the rider is down;
  - is handed to the shown rider's `riding`, which the picture's `riding_bones_of` draws from.
  - `OPENMX_RIDING_RAGDOLL=0` turns it off online as offline.
- `--trace-picture` prints `picture riding: tick T 0|1` per drawn frame: whether the first seat's rider is
  drawn from the riding ragdoll.
- Test `crates/openmx/tests/online_riding.rs`: two headless online shells on free09 (host holding Cross).
- Nothing in `../opentennis` covers this (tennis has no ragdoll on a vehicle).

## What the original does

The original has no online play. Offline its body actor steps the riding ragdoll at the end of the rider's
tick (S90, S110); the port's offline seat does the same.

## How it was proved

- `cargo test -p openmx --test online_riding` (via `./kit check`): both machines draw every frame from the
  riding ragdoll from its first step to the end, and the ride has 0 desyncs.
- Same test: the guest's idle seat's drawn nodes, at the ride's last tick, are within 0.032 ft of an offline
  ride's on the same idle pad (bound 0.1 ft).
- By hand, 240 ticks, host with gas: the drawn rider's middle stays 3.9–4.7 ft above the bike as it rides,
  3.72 ft standing, the same as offline standing (3.72). Idle seat nodes vs offline: within 0.01 ft from
  tick 30 on, 0.2 ft at tick 10 (the drawn riding ragdoll starts a few ticks later than offline's).

## Not verified / not 1:1

- Online riding body is not bit-exact to offline's: 0.03 ft apart on an idle seat. Likely causes, not pinned
  down: the online body machine is the session's (`Rider::body_frame`), not the shell's `Body::ride`; the
  seat stands in another bay; the drawn riding ragdoll starts a few ticks later (first grips drawn after the session's
  first tick).
- The drawn riding ragdoll steps once per frame in which the session tick moved. A frame that catches up several ticks
  steps it once, with one tick's `dt`. Not measured how often that happens online.
- The drawn riding ragdoll follows predicted ticks and is never rolled back. After a misprediction the body springs from
  where it was toward the corrected bike; a move over `STRAY` (10 ft) restarts it. Not tested with
  mispredicting pads (headless online pads are idle or scripted).
- A wreck online is still thrown from the kit's fixed riding pose, not from the riding bones as offline
  (the session has no riding ragdoll). Unchanged by this task, and not a drawn difference while riding.
- The drawn riding ragdoll's riding rng is its own, not the Sim's lent generator. Draw-only, so it touches no sim state.
- Riders on planes, cars and quads online: the drawn riding ragdoll only runs while the seat's rider has a ragdoll kit
  and is up; not checked on a seat switching machines.

## Next

Nothing for this task. A riding ragdoll inside the session (so online wrecks are thrown from it) needs the
chassis animation in the rolled-back state; not filed, as no task asks for it yet.
