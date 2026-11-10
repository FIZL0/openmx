# 1 — s12 ai tick (FINAL)

Task S281. 2026-11-10.

## What I did

- Added `ai::update` to `crates/game-sim/src/ai.rs`. It is the AI brain's per-tick update and runs its steps in the original's order. The steps it calls out to go through the `ai::Steps` trait, so the route follower, the copy and the rest can be plugged in.
- Ported the type-1 steep-climb rule (hold, steep, timer, speed) and the stuck clock in full.
- Added `ai::hand_off`, the interface's lever hand-off (vt+0xc) that gives steer/pitch/throttle/brake to the bike. It covers the back-up and swerve recovery overrides and the "not racing" all-brake case.
- Added `ai::assist_aim` and `ai::assist`, the route follower's tail. That is the rider assist that oracle.txt listed as "not ported" under ai_feed. It calls the assist through interface vt+0x18 under flag 0x800.
- Updated `research/oracle.txt`:
  - the rider sub-object at rider+0x10d0 and its vtable slots
  - four new named entries: `ai_rider_assist`, `ai_hand_off`, `ai_listener_walk`, `ai_update_base`
  - S281 notes on ai_update's order and the climb rule.
- Added `watch/s281-ai-tick.toml`, a 900-frame capture of the three free09 AI riders in state 44.
- Taken from opentennis: nothing. It has no AI riders with a comparable tick.

## What the original does

`ai_update` (`./kit fn ai_update`) takes the brain in a0 and dt in f12:

1. **Stuck clock.** Kinds 1, 2, 3 and 6 add dt to +0x3f0 when throttle +0xe0 > 0 and speed +0x80 < 10. At ≥ 5 it calls interface vt+0x40 (the stuck rescue, `./kit dis 0x00281460`) and zeroes the clock. In every other case it zeroes the clock.
2. **Mode +0xd0 == 5 (riding):**
   - With an interface, copy flag 0x20 (on the ground) into 0x400000, then call interface vt+8 (ai_feed).
   - Call the route follower (ai_tick_flag) and keep its answer.
   - Call the brain's own vt+0x90 (controls).
   - Call interface vt+0x28 (the target-approach set-up, `./kit dis 0x00280070`) if there is an interface and flag 0x200000.
   - Call the brain's vt+0xa8 (`./kit dis 0x00286b90`) under flag 0x4000.
   - Apply the climb rule.
   - Call interface vt+0xc (the hand-off, `./kit dis 0x0027fe90`).
   - Call the listener walk (`./kit fn 0x00133500`), which always answers 1.
   - The answer is route & walk.
3. **Any other mode:** `./kit fn 0x00282af0`.

**Climb rule.**
- If hold +0x390 > 0: hold −= dt and throttle = 0.
- Else, if forward.y +0xb4 > steep +0x394 and speed < +0x30c: pitch +0xf4 = 1, timer +0x398 += dt, and once the timer passes 5, hold = 5.
- Otherwise: timer = 0.

**Hand-off.** These are the bike's levers.
- With no brain, no race card at +0xf9c, or the race keeper's running bit clear: every lever is 0 except brake = 1, and the answer is 0.
- Otherwise:
  - steer = the swerve bar while the swerve timer > 0, else the brain's +0xf0
  - lean = +0xf4
  - on the ground: reverse with no gas or brake while the back-up timer > 0, else gas +0xe0 and brake +0xe4
  - in the air: only brake is zeroed.

The hit reaction (vt+0x3c, `./kit dis 0x002812b0`) sets both timers and draws the bar with `rand()`. The bike tick counts the timers down after the hand-off.

**Assist.**
- The yaw rate is set to mula(heading, w0) and then madd(yaw, 1−w0).
- On the ground, velocity x and z are set to mula(aim, w1) and then madd(vel, 1−w1).
- The aim is direction × target speed, from +0x1d0/+0x1d8 × +0x1e0.

**Comparing denormals.** The FPU flushes denormals in a compare, and the port's compares do the same (`op`). A denormal swerve timer counts as 0. That came up in the oracle.

## How it was proved

- **Function oracles on the PS2 ELF**, in `crates/game-sim/tests/ai.rs`, with 10,000 generated cases each. The inputs aim at every edge: denormals, ±0, timers on either side of 5, and speed at 10.
  - `the_ai_tick_matches_over_ten_thousand_ticks`: the whole of ai_update, with every call it makes hooked and played back from a script. It checks the order of the calls as well as the state.
  - `the_lever_hand_off_matches_over_ten_thousand_ticks`
  - `the_route_tail_and_the_rider_assist_match_over_ten_thousand_ticks`: the route follower's last stretch, then the assist on the result.
- **Live capture:** `the_free09_riders_ticks_are_the_originals` replays `context/capture/s281/ai-tick.csv` (900 rows, frames 2736..3635, 0 torn). That is per-tick +0xe0/+0xe4/+0xf0/+0xf4, the climb and stuck clocks, the flags and the bike's levers for the three AI riders.
  - Over 2595 rider ticks (74 of them swerving, 889 in the air), the port leaves the next row's levers, controls and clocks bit for bit.
  - The swerve timer is read at its phase, which is hit reaction → AI tick → countdown within one frame.

## Not verified / not 1:1

- **Steps that are hooked but not ported:**
  - the stuck rescue (interface vt+0x40)
  - the target-approach set-up (vt+0x28, 2404 bytes)
  - the brain's vt+0xa8
  - the listener walk's listeners
  - the non-riding update (0x00282af0)
  - the hit reaction that sets the back-up and swerve timers
  - the bike's countdown of those timers.

  `update` calls them through `Steps`, and the oracle proves only the order they are called in and what `update` does around them.
- **The route follower's body** is S282a/b. In the tick it is a `Steps` call.
- **The race keeper's running bit** (`*(card)+0x5c` bit 0) was not captured. The live test takes "racing" from card ≠ 0. The oracle covers the clear-bit case.
- **The climb rule and the stuck clock never fire on free09** (both timers stay 0). They are proved by the oracle only.
- **The assist is a no-op live**: both rider weights are 0 on free09. It is proved by the oracle only.
- **Ground-bit carry**: 2 rows (rider 1 frame 2879, rider 2 frame 3127). On the second airborne tick after a take-off, 0x400000 lags one tick in the row. Possible causes are the route follower's own writes to the flag word or a mid-frame read. Not resolved. The live test allows exactly these two.
- **Capture conditions:** taken at NominalScalar 0.3 under load. 16 two-tick steps and the repeated rows were skipped, not replayed.
- **MX vs. ATV's PC exe was not read.** The AI brain has no mapped PC twin in `research/mxvatv-pc.md` yet, and finding it would be a project of its own. Every branch of the three functions is covered by the PS2 oracle, which is the binding proof.
- **Not wired into the Sim.** Building `Brain` from the rider and hooking up the real steps is S12e.

## Next

S305 (filed) takes the unported callouts. S12e: wire `ai::update` into the Sim using the ported feed, route follower, controls and avoid. The unported callouts above (stuck rescue, approach set-up, hit reaction with its timers) are the remaining pieces of the opponent's tick.
