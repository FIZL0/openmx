# B33 — a struck prop searches in its strike tick

2026-11-03. Task: `./kit plan B33`.

One paragraph: what this task was, and how it came out. Details in the numbered files; the newest one's
suffix says whose move it is (FINAL · PART · READY).

In the original, the props node's collide walk re-reads its filed count. So a ball that the riding
bike's strike wakes, and files at the end of the list, has its own probe searched in the same pass:
its entries go after the strike's mirror, and its probe's mirror goes on the bike's list. The port
searched the props before the strike, so the ball's probe waited a tick (B28 only heard it). Now
`PropSet::strike_in` searches each object the strike filed straight away (`PropSet::search_late`,
in the pass's world and movers), and puts its probe's mirrors on the bike's list. Every bike lane
uses it: `Sim::step`, `step_all`, `step_bumped`, and the shared seats. To prove it, I made a new
pre-strike RAM image, f27a1 frame 2902 with ball 2 asleep, by re-running aim.py and knock.py
`--at 2902` from the same state. The run was word for word f27a1 again. From 2902, one tick now
matches 2903 bit for bit: ball 2's body, its whole list, the bike's list and the bike's velocity.
Done; details in `1-struck-prop-searches-in-its-strike-tick-FINAL.md`.

## Files

| file | what |
|---|---|
| `crates/game-sim/src/prop_contact.rs` | `Strike::by`, `PropSet::strike_in`, `strike_then` |
| `crates/game-sim/src/prop_set.rs` | `PropMovers::keys`, `PropSet::search_late` (the pass's own late search, shared) |
| `crates/game-sim/src/lib.rs`, `rider_contact.rs`, `prop_seats.rs` | every bike lane strikes with `strike_in` |
| `crates/game-sim/tests/knock_launch.rs` | `the_strike_tick_is_the_originals` (image 2902); B28's stand-in updated |
| `research/s195/knock.py` | `--at FRAME`: save slot 3 at that frame |
