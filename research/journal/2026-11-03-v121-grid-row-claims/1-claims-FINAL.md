# 1 — the grid rows a session with no AI rider claims (FINAL)

Task V121. 2026-11-03.

## What I did

- **Found the parting.**
  - A scratch probe (since deleted) printed, for every tick of S228a's clip-layer replay, four things
    side by side: the port's blend (entry point, clips, weights), the band, the eased axes, and the
    original's motion and machine words.
  - The worst spans (138164..138232, 139614..139689, 137948..137951) are all state 30 below the riding
    band: `Blend::Four` over the `turn_stop` and `turn_still` rows, or `Two` on the `turn_still` row.
  - On each of them the miss grows in step with how far the fore-aft axis (`lag_mix`) is from 0.5. The
    ratio is about 0.75 ft per unit of mix on the still row. The ramp is 0, and the weights are a pure
    function of the bit-exact axes.
  - On the riding row the same mix poses exactly (138320, mix 0.63, miss 0), so the weights and pairing
    are right. That leaves the clips.
- **The clips.** The port bound `turn_stop_Fwd`, `turn_stop_Mid` and `turn_stop_Bck` all to
  `turn_stop_mid.mot`, and `turn_still_Bck` to `turn_still_bck.mot`. These are V14's `airider.mcf`
  claims. The claim table and the original's code behind it are in V14's entry
  (research/journal/2026-10-17-v14-the-grid-rows-third-column) and in research/oracle.txt # V14.
- **The fix** (crates/game-render/src/anim.rs, crates/game-render/src/rider.rs):
  - The new `Set::alone` clears the claims read from the other `.mcf`s in the archive. A name then
    resolves through the set's own `.mcf`, which is the existing fallback in `Set::claim`.
  - `Body::load` calls it before any clip is bound. The port builds no AI rider body, so the player's
    set is the only one in the session.
  - `Set::load` keeps V14's archive order. game-sim's motion test builds its set through it and still
    proves V12's capture against the AI claims.
- **Tests** (crates/game-sim/tests/rider_anim.rs, crates/game-sim/tests/fly_and_collide.rs):
  - `the_fly_riders_drawn_lean_is_the_originals`:
    - `FLY_LEANING` goes to (1153, 1056) ticks.
    - The worst-tick bound goes from 0.8 to 0.4 ft.
    - It now asserts that no tick with the original's ramp at 0 is more than 0.02 ft off (the worst
      is 0.0148 ft).
  - Raised pins:
    - `TWO_P_DRAWN`: P1 9971 → 10481, P2 6502 → 6927.
    - `TWO_P_EASED_POSES`: P1 9968/10158 → 10496/10689, P2 6316/7812 → 6788/8365.
    - `DRAWN_TICKS`: (1793, 274) → (1793, 276).
    - The hard-turn lean ratio floor: 0.79 → 0.80.

## What the original does

- **The first set built claims the names.** The animation set is one cache for the whole game. The
  first `.mcf` to name a clip fixes its file (V14).
- **Who is on the track decides which set is first.**
  - V12's capture had AI riders on the track, and the AI's `airider.mcf` claimed first.
  - On S228's fly_and_collide capture (context/capture/s228/fly.csv) and C11's 2p capture
    (context/capture/2p/2p-eased.csv), two humans ride and there is no AI. Only `mrider.mcf` is
    there to claim.
  - With no AI, `turn_stop_Fwd` and `turn_stop_Bck` are their own files, and `turn_still_Bck` is
    `turn_still_Mid.MOT`. V14's journal predicted this under "a session with no AI riders would differ".

## How it was proved

`cargo test --release -p game-sim --test rider_anim --test fly_and_collide --test motion -- --nocapture`
passes, with nothing skipped: 17 + 3 + the motion suite. The whole gate ran in `agent-runner done`.

- **Clip layer**, the_fly_riders_drawn_lean_is_the_originals:

  | | before | after |
  |---|---|---|
  | leaning ticks within 0.01 ft | 937 | 1056 (of 1153) |
  | p90 | 0.081 ft | 0.0044 ft |
  | worst | 0.765 ft | 0.377 ft |
  | parting spans | 17 | 11 |

  - The stop and still spans are gone: 137948, 138164..138232, 139614..139625, 139630..139655 and
    139669..139689.
  - Every tick left past 0.02 ft has the original's ramp above 0. Most are 30↔35 restarts (136989,
    137017, 138378, 138800); the rest are the tails of other fades (136849 ramp 0.47, 139075 ramp 0.62,
    139759 ramp 0.40).
  - With the ramp at 0, the worst tick is 0.0148 ft off. It sits in the two small air-row spans
    137221 and 137239..137243, where mix is about 0.7. The test bounds this at 0.02 ft.
  - A fade crosses out of the restart's snapshot. The original takes that snapshot off the riding
    ragdoll's written-back nodes (V109, F12). fly.csv has no node columns, so this replay snapshots
    the clip pose instead.
- **C11's 2p capture** (V90's replay, joints within 1e-4):
  - P1 goes from 9971 to 10481 ticks, P2 from 6502 to 6927.
  - Seeded off the original's eased axes: P1 10158 → 10689, P2 7812 → 8365.
- **The drawn layer** (the riding ragdoll, the_drawn_rider_leans_as_the_original_through_the_turns):
  - Lean as a vector: p90 0.425 → 0.232 ft, p99 0.766 → 0.607 ft.
  - Hard-turn lean ratio: 0.795 → 0.807.
  - The worst tick is unchanged (137240, before the crossup, S253's under-lean).
  - The "before" figures came from the same test run with the `set.alone()` line taken out.
- **The other column, unchanged.** game-sim's motion test (V12's capture, AI on the track) builds its
  set with `Set::load` and passes as before.

## Not verified / not 1:1

- **Which body the original builds first was not read off its spawn path.** The rule "AI riders out
  → AI claims, none → the player's" comes from two kinds of capture that agree with it on poses. It
  was not read from the code that spawns the bodies.
- **The port's shell has no AI rider bodies**, so `Body::load` always claims alone. If AI riders are
  ever drawn, the player's body in such a session needs `Set::load`'s archive order instead (V12).
- **Not checked across sessions.** Whether the original's cache is emptied between levels was not
  checked. A session with AI followed by one without could keep the AI claims there. The port has no
  such carry-over.
- **The fades are not checked here.** Of the 97 ticks still past 0.01 ft, all but the air-row
  residual below are restart fades. This capture cannot check those (no node matrices). V109's snap
  capture covers that layer on the 2p tape.
- **An air-row residual of up to 0.0148 ft is not explained.** It is on state 35 with the ramp at 0
  and mix about 0.7, at 137221 and 137239..137243. Its clips are the same in both `.mcf`s, so it is
  not this claim. I did not trace it.
- **S253's 20% under-lean in hard turns is unchanged** in kind. It moved only from 0.795 to 0.807.
- The shell was not run, and nothing was looked at on screen.
