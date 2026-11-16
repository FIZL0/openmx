# 1 — one generator for a race (FINAL)

Task S333. 2026-11-16.

## What I did

- `crates/game-sim/src/tick.rs`:
  - `Sim::think`: with a race in the Sim, the AI brains draw on `race::Race::rng`, not on
    `ai::Lane::rng`. With no race (a free ride, C33's free09 replay) the lane's own is used, as before.
  - `Sim::spawn_ai`: with a race in the Sim, the race takes the generator the spawns left. So the
    load's draws and the race's draws are one sequence.
- `crates/openmx/src/race_start.rs`: the shell's `spawn_ai` draws the roster from the race's
  generator when the Sim has a race (it always has, where `main.rs` calls it). `AI_SEED` is only the
  fallback now.
- Doc lines on `Sim::ai` (lib.rs), `Race::rng` (race.rs) and `ai::Lane` (ai.rs).
- No wire or hash change: the race's generator was already on both; the lane is on neither.
- Tests (`crates/game-sim/tests/race.rs`, header item 51):
  - New: `a_races_ai_riders_draw_on_the_races_generator_after_the_race`.
  - S13d's live hold now asserts the over tick's tie draw is 0 draws on (the frame's first `rand()`).
    Its comment said the riders' draws came first; that was wrong, and is corrected.
- Nothing taken from opentennis: it has one generator and no AI lane to share it with.

## What the original does

- One `rand()` for the whole game (`rng.rs`'s header). The race manager, its flash child, and every AI
  brain draw on it.
- The frame order: the race manager's tick (`./kit fn 0x002fcff0`) runs ahead of the riders in the
  scene walk (S13b, `tick.rs`'s note), and its flash child ticks last in it (S321). The AI brains think
  in the rider loop after it (S323).

## How it was proved

- **A live capture** (S13d's `context/capture/s13d/sx01-finish.jsonl`, per-row `rand` state). At the
  over tick (frame 2567) the race's tie draw is found 0 draws on from the row before's state, and the
  tick's end state is 4 draws past it. So the race manager's draw is the frame's first, and the other
  callers (AI riders among them) draw after it on the same generator. Now asserted in
  `the_results_hold_s13ds_live_sx01_finish` (probed: "tie draw 0, after Some(4)").
- **The Sim's order**, `a_races_ai_riders_draw_on_the_races_generator_after_the_race`, 600 ticks:
  - a human and one AI rider on a flat floor; the race long started, so it draws only its flashes;
  - each tick: the lane's own generator does not move;
  - the race ticked alone from the tick's start gives where the flashes end. The same tick with no race,
    and the lane's generator put there, ends on the race's state with the AI rider in the same place.
    So the race's draws come first, then the brains', on one sequence;
  - `spawn_ai` leaves the race on the spawns' end state;
  - probed: 1,100 flash draws, 20 brain draws, 4 ticks with both.

## Not verified / not 1:1

- **No whole-race hold of the shared generator against the capture.** The sx01 capture has no AI
  rider state (cards only), so the Sim cannot replay its AI riders' draws. Only the order (race manager
  first) and the race's own draws (S13d, S321) are held live.
- **The order among the frame's other callers is not proved.** The capture shows how far the generator
  moved per row, not who drew. The brains' order among themselves is S323's (the rider loop, last
  player first), not re-checked here.
- **The flat-floor AI rider draws little** (20 draws in 600 ticks, no seeker world). It shows the
  order, not every brain draw site.
- **Other generators in the Sim are not shared.** Only the race and the AI lane were asked for. The
  riding ragdoll (`ragdoll_riding`), the rider and vehicle animation, the flier (`flight`), the quad
  and the car each still keep a generator of their own. Filed as S336.
- **The load's order is the port's guess.** The shell draws lay-out (gate call, lanes) first, then the
  roster and the spawns, on one generator. The original's load order of those draws was not read, and
  its seed comes off the wall clock.
- **The lane's generator goes stale in a race.** It keeps the spawns' end state and is not drawn.
  Nothing reads it in a race; a race removed mid-ride would go back to it.
- `kit done` runs the gate; the openmx shell change was not run headless here.

## Next

Nothing.
