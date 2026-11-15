# 3 — effects leaf (FINAL)

Task F31b5n. 2026-11-14. Reopened after F57a landed. Done.

## What I did

- **Checked what F57a left.** F57a already did what entry 2's "Next" asked: `PartWalk` runs the first
  emitter kind's other branch through `emitter::other`, and `flip_replays_with_every_port_at_once`
  asserts that no part-walk hand-out draws `rand` (the list is empty). So every draw on `rand` along the
  part walk's effects leaf and both emitter kinds is now `game_sim::rng`'s.
- **ride-2 holds the same check.** `ride_replays_with_every_port_at_once` now also asserts that no
  part-walk hand-out drew `rand` (before, only flip-1 asserted it).
- Nothing from opentennis: it has no particle emitters.

## How it was proved

- `flip_replays_with_every_port_at_once` and `ride_replays_with_every_port_at_once`: C15 word for word on
  every row, and the whole RAM matches the handing run's after every frame, `rand`'s state included, apart
  from the HUD's, race-progress and cue words those tests already allow for. Both pass.
- `flip_needs_the_part_walks_effects` and `ride_replays_without_the_part_leaves` pass.
- Gate: `./kit check --all` in `./kit done`.

## Not verified / not 1:1

- **The emitters' hand-outs that draw no `rand` are still the original's**: notice, light/douse, send,
  place, owner, follow, probe, settle, bind. The task asks only for the `rand` draws. The `PartWalk` watch
  asserts each of these draws no `rand` and writes no bike word.
- **The Rust `rand` has no oracle of its own here.** It is held to the replays, where `rand`'s state must
  match the original's after every frame. `game_sim::rng` has its own tests.
- **The ports are not wired into the shipped game.** Only the tests call `effects::step`, `emitter::step`,
  `emitter::other` and `ring::step`. The pure frame (F31b5) has to wire them in.
- **Only two captures (flip-1, ride-2) test the whole path.** Emitter branches those rides never reach
  are covered only by the F57/F57a/F59 oracles.
