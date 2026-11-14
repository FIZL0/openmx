# 1 — race mode (PART)

Task P313a. 2026-11-13.

## What I did

- Checked the dependency first. S12e (AI riders in the Sim) is still open on main. Main already has
  the AI lane's plumbing from S323: `game_sim::Sim::ai: Option<Box<ai::Lane>>`, `Sim::think` called
  per player in the frame (crates/game-sim/src/lib.rs, the `self.think(i, dt, world)` line), and
  `ai::Pilot::spawn`. But nothing outside a test ever builds an `ai::Lane`.
- S12e's branch (task/S12e, at fc272078e and about to land) builds the lane by hand from C33's RAM
  image, inside `crates/game-sim/tests/ai/race4.rs` (`sim.add_player` per racer, then
  `sim.ai = Some(Box::new(Lane { seek, tables, rng, pilots }))`). It also files **S331**: build
  `ai::Lane` from the roster at a real map load (Pilot::spawn per racer, seats, the seek world, grid
  slots) when the shell boots a race with racers > 1.
- So the shell has no library call yet that gives it AI riders to put in race_start::lay_out's lanes
  and race_start::laps' cards. If this task wired COMPUTER_CARD entries now, they would belong to bikes
  that never move, and it would cut across S12e's code while that task is still in flight. Blocked
  `--after S12e`.

## What the original does

Not read this session (nothing was ported).

## How it was proved

Nothing was proved: nothing landed.

## Not verified / not 1:1

- The whole task: no AI riders on the shell's grid and no AI cards yet. The reason is that the Sim
  cannot spawn an AI lane outside a test until S12e lands (and S331, which S12e files, or this task
  doing S331's spawn in the shell).

## Next

Once S12e has landed: check whether S331 has landed too. If not, build the lane in the shell from the
roster as race4.rs does: `ai::spawn_roster`, then `Pilot::spawn` per AI racer, `Sim::add_player` at
each of lay_out's lanes 1..racers, and `Sim::ai`. Then add a `race::COMPUTER_CARD` card per AI rider
in race_start::laps' `riders`. Test it in crates/openmx/tests/race_menu.rs: a `--race --racers 4` ride
should have four cards and three AI riders behind the gate. Check it in a release build.
