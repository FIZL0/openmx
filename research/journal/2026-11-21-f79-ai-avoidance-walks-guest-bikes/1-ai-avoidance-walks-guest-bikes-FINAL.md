# 1 — ai avoidance walks guest bikes (FINAL)

Task F79. 2026-11-21.

## What I did

- `Sim::ridden_guests` (game-sim lib.rs): the other local seats' bikes (`ai::Ridden`), not hashed or sent;
  the snapshot decode keeps this machine's.
- `Sim::think` (tick.rs) appends them to its scene bikes after the Sim's own players', as the race list
  (P350a) puts the other seats' cards after the Sim's own.
- The shell (main.rs `race_guests_in`, before seat 0's pass) fills it from seats 2+ (`Ridden::of`, so a
  downed or ghost rider's bike is left out, as the Sim's own are) whenever seat 0's Sim has AI riders,
  and empties it otherwise. Scope choice: race or not (the brief names a race). The original's walk holds
  every bike in any ride, so a split-screen free ride with AI riders gets them too.
- The snapshot decode (snapshot.rs, outside the task's files) gets two lines so `Sim` still builds.
- Nothing taken from ../opentennis (it has no AI scene walk).

## What the original does

Its avoidance's scene walk hands out everything in its one moving structure, every rider's bike frame
among it (S326, tests/ai/moving.rs). In split screen every local rider's bike is in that one structure,
so the AI riders' walk lists them all. Every bike collidable carries the AI_IGNORE bit, so the walk
lists them and the feelers never sweep one.

## How it was proved

- `./kit check -- -p game-sim --test ai` passes (the S326 moving-world tests among them).
- No new test: since the bikes are AI_IGNORE, adding them changes only the scene's walked list, which
  `Sim::think` drops. The brain's controls are the same with or without them, so no test outside the
  Sim can see it. Making the walk visible would mean changing ai.rs, outside this task's files.

## Not verified / not 1:1

- The order of bikes in the walk in split screen: the port puts the other seats' bikes after seat 0's
  Sim's players (AI riders among them). The original's moving-structure order for several local seats
  is not captured. It changes no control, as no bike is swept.
- No capture of the original in a split-screen race with AI riders. None exists, and this task did not
  record one.
- No test reads the walked list through `Sim::think`, for the reason under How it was proved.

## Next

none
