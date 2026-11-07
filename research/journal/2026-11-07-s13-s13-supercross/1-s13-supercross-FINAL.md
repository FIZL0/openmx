# 1 — s13 supercross (FINAL)

Task S13. 2026-11-07.

## What I did

- **Split S13.** The whole task (activity class, gate drop, laps, results, sx01 data) is five pieces,
  each needing its own proof, several of them live captures of a supercross race that nothing in the kit
  can reach yet (no pad: menus go by generated `.p2m2` tape). Filed as S13a (the activity class), S13b
  (the gate's drop and wiring the watch into the Sim), S13c (laps and checkpoints), S13d (results),
  S13e (sx01.res's race data). This session did the piece all of them stand on: the gate.
- **`crates/game-sim/src/race.rs`** (new, mounted in `lib.rs`): `Gate` (flags, centre, normal, across,
  width, height, reach²), `Gate::crossed` → `Crossing { Not, Through, Beside, Open }` with the
  original's codes 0/1/2/4; `Seen` (a score card's now/last positions), `Gate::watch` — the race
  manager's start-gate watch for one entity — returning `Caught { push, penalty }`; `gate_push_back`
  and `gate_penalty`, the two `rand()` draws.
- `research/oracle.txt`: `race_manager_tick` named (the test calls it by name), and the activity
  class's vtables and own slots listed under `activity_supercross` for S13a.
- Nothing taken from opentennis: it has no gates, laps or races of this kind.

## What the original does

The crossing test (`./kit fn 0x00246720`, `start_gate_crossed`) is one function behind a dozen callers
(the race manager's start gate, the score cards' line tests, others in the 0x0026xxxx range). Given a
gate and two points `from`, `to`:

1. null point → 0; gate flag 1 clear → 4.
2. reach² (+0x7c) > 0 and |centre − from|² > reach² → 0 (FPU `mula/madda/madd`, `c.ole`).
3. `d = from − to` (VU `vsub.xyz`), `len = sqrt(d·d)`; len ≤ 0 → 0.
4. `t = ((centre − to)·normal) / len`; `t < 0` or not `t ≤ 1` → 0. Flag 0x10 (whole plane) → 1.
5. width (+0x70) ≤ 0 → 0. `q = to + d·t − centre` (VU `vmulx`, `vadd`, `vsub`);
   `s = 2(q·across) / width`. Flag 8 and `s < 0` → 2; `s > 1` or `s < −1` → 2;
   `q.y − across.y·s > height` (+0x74) → 2.
6. Flag 4 → 1; else `d·normal > 0` → 1, otherwise 0.

The race manager's tick (`./kit fn 0x002fcff0`) runs, while its gate (+0x3dc) has flag 1, over every
score card (+0x84, next +0x1c8) and each card's entities (+4, next +0x1c): message vt+0x94(1) gets the
entity's object; card +0x70 = +0x60, +0x60 = object +0x30; `crossed(gate, +0x60, +0x70)` — the new
position first — and on 1: message 2 with object +0x20's xyz times `-5 - 5·(2⁻³¹·rand)` (w kept), then
card +0x1c0 = `2⁻³¹·rand + 1`. The constants are set in its loop preamble (`lui` 0x3000 / 0x40a0 /
0xc0a0 / 0x3f80 → 2⁻³¹, 5, −5, 1).

## How it was proved

`crates/game-sim/tests/race.rs`, both oracles in the interpreter, no emulator:

- `the_crossing_test_matches_over_twenty_thousand_steps`: the original function on 20,000 generated
  gates and steps against `Gate::crossed`, aimed at each branch (flag subsets with junk upper bits,
  reach zero/negative/positive, zero-length and tiny steps, `t` at 0, 1 and just past, widths ≤ 0,
  side at ±1 and just past, the height edge, both directions), with denormals, Inf/NaN patterns,
  signed zeros and random words mixed into every field. Every answer 0/1/2/4 comes up > 500 times.
- `the_start_gate_watch_matches_the_race_managers_tick`: the whole race manager tick on 10,000
  generated races (1–3 cards × 1–2 entities, gate live and not, the generator anywhere in its chain),
  the message slot hooked and the score manager's tick after it stubbed; compared on every card's
  +0x60/+0x70/+0x1c0, every push message (entity and quad, bit for bit) and the generator's state.
  > 500 riders caught.
- Mutants: `side <= 1` → `side < 1` and dropping the height term both turn the tests red.

## Not verified / not 1:1

- **The rest of S13 is not done** — the activity class, the gate's drop, lap and checkpoint counting,
  the results and sx01.res's data are S13a–S13e. Nothing here is wired into the Sim yet: `Gate::watch`
  has no caller, so a rider can still roll through a start gate the port does not have (S13b).
- **Null points.** The original answers 0 for a null `from` or `to`; the port takes values, so that
  case cannot arise and is not represented.
- **The rest of the race manager's tick** — the +0x4c0 countdown (its sound/menu calls), the +0x410
  countdown (the HUD fader) and the +0x470 arming of it — was held at zero in the oracle and is not
  ported; it is S13b's.
- **What the push message does to the bike.** The oracle hooks vt+0x94; what message 2 does to a
  bike (a move by the vector, by its name) was not read, and is S13b's when the watch is wired in.
- **What gates exist on a real track.** The generated gates are shaped like the fields read, not
  read off sx01; which flags the start gate and the lines actually carry is S13e's.
- **Which callers of the crossing test are which.** Only the race manager's caller was read; the
  other eleven (score cards' lines, stunt/checkpoint activities) are assumed to be S13c's.

## Next

S13a–S13e in order; S13e (the track's gate data) and S13b (wiring the watch) are the ones a race needs
first.
