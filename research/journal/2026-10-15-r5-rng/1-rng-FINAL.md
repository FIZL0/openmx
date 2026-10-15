# 1 — rng (FINAL)

Task R5. 2026-10-15.

## What I did

- Looked for the generator the task's brief described — a 32-bit multiply by a constant and a stored seed
  — and did not find one, because there isn't one: the game has no RNG of its own. It calls the C
  library's `rand` from 224 sites spread across physics, AI, menus and presentation, and seeds it from 35
  more. That *is* the "cross-referenced from everywhere" function; the multiply is 64-bit, not 32.
- Ported it as `crates/game-sim/src/rng.rs`: `Rng::rand` (the library's `rand()`), `Rng::seed` /
  `from_seed` (`srand`), `from_state`/`state` for mid-chain snapshots, and the two shapes its call sites
  wrap it in — `unit()` (the random float) and `index(count)` (a uniform index into a list).
- Proved all four against the game's own instructions with the oracle: `crates/game-sim/tests/rng.rs`.
- Named `srand`, `rand_state`, `random_list_pick` and `random_time_step` in `research/oracle.txt`
  (`rand` was already there from R2).

How the search went, in case the next person needs the same trick:

- Ranked every function by its number of call sites (`boot.xrefs.tsv`, `*CALL*` rows) and intersected
  that with functions under 200 bytes. The library's `rand` is in the top thirty with 226 rows.
- Scanned the whole boot image for `mult`/`multu` whose operand is a constant built with `lui`+`ori`
  (script kept out of the repo; it is ten lines of `struct.unpack` over the `PT_LOAD` segments). Every
  hit is a division magic — `0x66666667`, `0x51eb851f` and friends — so there is no 32-bit LCG anywhere
  in the executable. That is the negative result that made the C library's generator the answer rather
  than a guess.
- `grep -c` on the decompilation: `4.656613e-10` (2⁻³¹) appears at 199 call sites, which is what says the
  "random float in [0,1)" shape is the one the game actually uses, and that it is inlined everywhere
  rather than being a function.

## What the original does

`./kit dis` over `@srand`..`@rand`+0x60 is the whole story (both are tiny):

- **`srand(seed)`** — zero-extends the seed from 32 bits (`dsll32`/`dsrl32`) and stores it as the whole
  64-bit state. So seeding can only ever reach a quarter of the state space, and a freshly seeded
  generator's first number is a plain function of the seed.
- **`rand()`** — state × `0x5851f42d4c957f2d` + 1 (the multiplier is built in four instructions across
  the call's delay slots; the 64-bit multiply itself is a library helper), stores the new state back,
  then returns the *top* half of it with the sign bit masked off: `(state >> 32) & 0x7fffffff`. Note the
  order for anyone comparing against a RAM dump: the state is advanced before the value is taken out of
  it, so the word in RAM after *n* calls is the state the *n*th result came from.
- **The state** lives in the library's static per-thread block, reached through a pointer in the image's
  data (`@rand_state` records the word itself). The image links it holding **1**, so a game that never
  seeds still has a defined sequence — which is the seed the proof below starts from.
- **The float shape**, inlined at 199 sites: `cvt.s.w` the result, then `mul.s` by `0x30000000` (2⁻³¹,
  exact). The conversion is where all the error is: `rand()` returns up to 2³¹−1, far past the 24 bits an
  `f32` holds, and the EE chops the low bits *toward zero* where IEEE would round. One bit there is the
  difference between picking entry 3 and entry 4.
- **`random_list_pick`** (`@random_list_pick`, with two instruction-identical siblings at `0x003aaa30`
  and `0x003aaab0` — a template instantiated per list type) is the uniform pick: convert the count,
  multiply by the random float, `cvt.w.s` to truncate, report the index through an out-pointer if one was
  passed, and return that entry.
- **`random_time_step`** (`@random_time_step`) is `t + 7.0 × max(0.1, rand()×2⁻³¹)`; its one caller
  chains it to build a run of increasing event times. It is not ported — it is used in the test as the
  cheapest *exact* view of the random float, since its result is an exact function of that one float.

## How it was proved

Function oracle, no emulator, all four in `./kit check` (`crates/game-sim/tests/rng.rs`):

1. `ten_thousand_calls_from_the_dumped_seed` — 10,000 consecutive `rand` calls from the seed dumped out
   of the program image, comparing the returned number *and* the 64-bit state word left in RAM after
   every single call. The image's initial state is also asserted against the port's constant, so the
   "unseeded" sequence is pinned to the image rather than to my reading of it.
2. `seeding_lands_on_the_same_sequence` — 100 seeds (0, 1, 2, both ends of the range, `u32::MAX`, then 94
   generated) through the original's own `srand`, each followed by 100 calls: 10,000 more comparisons,
   with the state word left dirty beforehand so a port that forgot to clear the state's high half could
   not pass by accident.
3. `the_uniform_index_matches_the_list_pick` — 10,000 picks. Half use a real eight-entry table, so the
   returned entry is checked as well as the index; half use counts up to 2²³ with the entries based at
   zero, which makes the index carry twenty-three bits of the random float. That is what turns this into
   a proof of the chopped integer-to-float conversion and not just of the truncation.
4. `the_random_float_matches_bit_for_bit` — 2,000 values through `random_time_step`, compared as bit
   patterns.

Plus four unit tests in the module that need no disc: the 2⁻³¹ literal's bit pattern, the first value of
the unseeded sequence worked by hand, that seeding clears the state's high half, and that 10,000 results
stay inside `0..=RAND_MAX`.

Then I broke the port on purpose twice, to check the proofs are load-bearing:

- increment 1 → 3: all four oracle tests fail.
- the integer-to-float conversion via the host's `as f32` (round-to-nearest) instead of the EE's chop:
  the two float proofs fail, the two integer ones pass. Exactly the split you would want, and the
  argument for `ps2-float` in one line.

## Not verified / not 1:1

- **The seed was dumped from the program image, not from a running game.** PCSX2 cannot start in this
  session — its Qt platform plugin has no display to attach to (`XDG_SESSION_TYPE=tty`, no Wayland or X
  socket, and the `offscreen` plugin is not installed) — so there is no live RAM and no save state to
  read. The state word is statically initialised, so the image *is* where a console's first seed comes
  from, and proofs (1) and (2) walk 20,000 mid-chain states between them. What is left unchecked is only
  that a running game's state still lives at `@rand_state`: the pointer it is reached through is
  statically initialised and nothing in this task suggested a second thread, but I did not confirm it
  against a live read.
- **What the game reseeds *with* is unknown.** 35 call sites call `srand` and I did not look at what they
  pass — a frame counter, a timer, a button pattern. A replay fixture needs that answer, so whichever
  task first records one should chase those call sites.
- **`index(count)` is proved for counts in 1..=2²³ only.** Above that the original would read its entry
  from an address outside the 32 MB of RAM, so the oracle cannot call it there, and a count of zero or a
  negative count is not something the original's callers can produce. The port follows the instruction
  sequence (so it inherits the FPU's saturating `cvt.w.s`), but that path is unproven.
- **The 10% of random floats below 0.1 are hidden by `random_time_step`'s clamp**, so for those the
  float is proved only to the 23 bits that proof (3) resolves, not bit-exactly.
- **The two sibling instantiations of the list pick** (`0x003aaa30`, `0x003aaab0`) were compared against
  the decompilation of the one I proved and are identical C; I did not run them through the oracle.
- **The library's 64-bit multiply helper is not named or ported.** The oracle executes the real
  instructions, so nothing is assumed about it, but the port computes the multiply with a `u64` —
  equivalent here because the result is taken modulo 2⁶⁴, and the 20,000-state comparison would have
  caught it if not.
- **`Sim` still carries the scaffolding's `rng: u32` field.** Wiring the simulation state to `Rng`
  belongs to whichever task first needs chance inside `tick`; doing it here would have changed the state
  hash and the example test for no proof gained, and would have fought other agents' merges.

## Next

Nothing waiting. The generator is available as `game_sim::rng::Rng` for any task that needs chance, and
the first task that records a session should find out what the 35 `srand` sites seed from, which is the
one gap above that a later proof actually depends on.
