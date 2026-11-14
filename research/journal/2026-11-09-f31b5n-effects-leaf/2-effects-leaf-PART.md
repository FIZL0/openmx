# 2 — effects leaf (PART)

Task F31b5n. 2026-11-14. Reopened after F56, F57, F58 and F59 landed. Blocked again, this time on F57a.

## What I did

- **Checked what was still the interpreter's.** F56/F58 put the effects items' turning virtuals and steps
  in `PartWalk`, F57/F59 both emitter kinds' own work. One thing was left on the path from the part walk
  to the bike: the emitters' draws on the C library's `rand`. Those still went to the original's `rand`.
  That is the one channel F31b5j found the part walk reaching the bike through (the trick hold's smooth
  noise is built off `rand`).
- **`rand` is now the port's** (tests/mxvatv_bike_ride.rs `rand_in_ram`): `game_sim::rng::Rng` resumed from
  the state doubleword in RAM (the one `rand_state` reads), one `rand()`, the state written back. The
  emitter port's `Call::Rand` is answered by it in `PartWalk` and booked as `the update's emitter rand
  draws` (flip-1: 636 draws).
- **Assertions**: `walk_served` now fails if an emitter's `rand` is handed to the interpreter.
  `flip_replays_with_every_port_at_once` asserts the port drew `rand`, and that the only part-walk hand-out
  that still draws `rand` is the first emitter kind's other branch (`emitter other kind`, 128 steps on
  flip-1). That branch is open task **F57a**.
- Nothing from opentennis: it has no particle emitters.

## What the original does

`rand` (`./kit fn 0x003561a8 -p atv`) is newlib's: the reent block's 64-bit state times the LCG multiplier
plus 1, the top half masked to 31 bits. That is `game_sim::rng::Rng::rand`.

The emitter's other branch (`./kit fn 0x0050db60 -p atv`, 2872 bytes and ten callees) is taken when the
emitter definition's option bit 0x40 is set (2 of the 11 emitters). It draws `rand` itself.
research/oracle.txt `# F57` has the details. Porting it is F57a.

## How it was proved

- `flip_replays_with_every_port_at_once` and `ride_replays_with_every_port_at_once` with the emitters'
  `rand` in Rust: C15 word for word on every row, and the whole RAM the handing run's after every frame,
  `rand`'s state included, apart from the HUD's and race-progress words those tests already allow for.
  Both pass.
- `flip_needs_the_part_walks_effects` and `ride_replays_without_the_part_leaves` were rerun.
- Gate: `./kit check --all` in `./kit done`.

## Not verified / not 1:1

- **The task's acceptance is not fully met.** The first emitter kind's other branch is still run by the
  interpreter (`Call::Other`), and it draws `rand` on its own 128 times on flip-1. So "the emitters Rust's,
  as far as their draws on rand" holds for every emitter path except that one. Porting that branch is F57a
  (open, about 5 KB of VU0 code with its callees), and this task waits on it.
- **The emitters' other hand-outs are still the original's**: notice, light/douse, place, owner, follow,
  probe, settle, bind. None of them draws `rand` or writes a bike word (the `PartWalk` watch asserts both).
  The task asks only for the draws on `rand`.
- **The Rust `rand` has no oracle of its own here.** It is proved only by the replays, where `rand`'s state
  must match the original's after every frame (636 draws on flip-1). `game_sim::rng` has its own tests.
- **The ports are still not wired into the shipped game.** Nothing outside the tests calls `effects::step`,
  `emitter::step` or `ring::step`. The pure frame (F31b5) has to wire them in.

## Next

When F57a lands: answer `Call::Other` in `PartWalk` with its port. Then change the
`flip_replays_with_every_port_at_once` assertion so no part-walk hand-out draws `rand` (the expected list
becomes empty), rerun both every-port replays, and finish this task.
