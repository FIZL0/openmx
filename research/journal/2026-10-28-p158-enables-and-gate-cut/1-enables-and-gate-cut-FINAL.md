# 1 — enables and gate cut (FINAL)

Task P158. 2026-10-28.

## What I did

- Looked for what clears the bike's steer and throttle enables (`+0x610`/`+0x614`) by scanning every
  store in the ELF (`llvm-objdump` of boot.elf, every `sw` at `0x610..0x61c` and every address formed
  as `base + 0x610/0x614`, each mapped to its function). **Nothing clears them during a ride** — see
  below. So the start-line latch never fires in the shipped game.
- Found what the bike tick's `+0x1c0` gas cut reads: the score card's start-gate penalty, set by the
  race manager's tick when a rider crosses its closed start gate (a jump start): the rider is pushed
  back and loses the gas for 1–2 s.
- Ported all three anyway, since the task asks for them "when a race mode or a scripted lock needs
  it" and the limiter half is cheap to prove exactly:
  - `ride::Lock` (in `Held`, wired in the snapshot): steer / throttle locks, last frame's steer lock,
    the latch. Kept as *locks* (inverted enables) so `Held::default()` rides as before.
  - `Held::step` runs the limiter's lever half on them: latch set on the frame steering goes off,
    let go when it comes back or with throttle on + gas asked + clutch lever 0; while set, brake lever
    1.0 and reverse 0; throttle off holds the gas at 0; steering off holds the sideways lean at 0.
  - `Controls::gas_cut` (the rider's, filled from `Score::gate_penalty > 0` as the manager's last tick
    left it) and `Controls::gate_cut()`, applied in `Ride::frame_searched` after the pad read's tail.
  - `Score::gate_penalty` (card `+0x1c0`) run down at the top of `Score::grow`, every card, before the
    combo/shut early return.
  - The physics' reverse input now reads `held.reverse` (which the latch can zero) instead of
    `c.reverse`; identical while no lock is set.
- Nothing in the port sets a lock or the penalty yet: no race mode exists. With them all clear the
  ride is unchanged (gate green, replay hashes untouched — `Lock` and `gate_penalty` are not in the
  hash walks).
- Pins moved on purpose: `Ride` size 2440 → 2448 (tests/ride.rs), `Sim` 11760 → 11768
  (tests/hash.rs), the riders' resync bytes 13463 → 13495 (tests/platforms.rs, digest re-pinned).
  No per-tick hash moved.
- Filed P188: the race's start gate (the penalty's setter) for when a race mode lands.
- opentennis: nothing comparable (no lever limiter there); nothing taken.

## What the original does

- Writers of bike `+0x610..+0x61c`: the base vehicle ctor (`./kit fn 0x002472c0`, all four 0); the
  vehicle builder (`./kit fn 0x00249cb0`, the stores at 0x0024a0f4..0x0024a138 — P77 called these "a
  message handler"; they are the build-up storing part pointers, non-zero); the set-up (0x0024f5d4:
  1, 1, 1, 0); the put-back (0x0027f61c: `+0x610 = 1`); the limiter (`./kit fn 0x00251a50`) for
  `+0x618`/`+0x61c`; the bike tick's copy (0x0027ca2c/0x0027ca38) through `+0x1068`/`+0x106c`, which
  are only ever 0 (ctor) or the bike's own `+0x610`/`+0x614` (0x0027cab4). Other hits at those offsets
  are other objects (a DMA packet builder 0x001e0070, a pad map 0x0011d670, a float block 0x00393800).
- The limiter's lever half: oracle.txt P77 (unchanged) + P158.
- The penalty: `./kit fn 0x002fcff0` (race manager tick, vtable 0x004587a4): while its start gate
  (`+0x3dc`) has flag 1, each card on `+0x84` whose entity crossed the gate (`./kit fn 0x00246720`
  returns 1) gets pushed back and `+0x1c0 = rand/2^31 + 1.0`. `./kit fn 0x0025ffc0` runs every card's
  `+0x1c0` down by dt while > 0, called first in the score manager's tick (`./kit fn 0x0025fcd0`) unless
  the manager's flag 8. The card join (`./kit fn 0x0025f370`) zeroes it.

## How it was proved

- `crates/game-sim/tests/levers.rs`, `the_levers_are_the_originals_switches_and_walk`: the 20,000
  generated cases now also generate the four enable words (each on 2/3 of the time, latch 1/2), the
  gate cut (1/5, applied between `pad_to_gesture` and `bike_input_limiter` as the bike tick's one
  store), the sideways lean and its rate; compared bit for bit after the limiter: `+0x640`, `+0x648`,
  `+0x618`, `+0x61c`, `+0x670` besides the old eight. Coverage asserted (latch set > 500, let go > 500,
  cut > 1000, throttle off > 1000). Checked to bite: inverting the latch's set test reds it.
- Same file, `the_gate_penalty_runs_down_as_the_originals`: the original's card-list run-down in the
  interpreter on 10,000 generated cases × 3 cards (0, negative, exactly dt, up to 2 s) against
  `Score::grow`. Checked to bite: `<=` for `<` reds it.

## Not verified / not 1:1

- **The cut's order against the pad read** is read from the code, not proved: `pad_to_gesture` is
  reached through a vtable slot (thunk 0x0027ff90) whose caller I did not trace; the port (and the
  test harness) put the cut after the pad read and before the limiter, which is the only order in
  which the store at 0x0027ca18 does anything.
- **The cut itself in the harness** is the harness writing `+0x63c = 0`, not the bike tick running:
  the tick (0x0027c890) is too big to call in isolation. It is one store.
- **No live capture**: nothing in freeride or the freestyle event sets a lock or the penalty, so no
  save state or recording exercises them; the proof is the oracle only.
- **The penalty's setter (the race's start gate) is not ported**: no race mode exists. Its push-back,
  the random length (rand()/2^31 + 1, from the game's RNG) and the gate-crossing test are documented
  in oracle.txt (P158) for the race task. Filed as a follow-up.
- **The score manager's "stopped" flag (8)** skips the run-down in the original; the port's `Score` has
  no stopped state, so `grow` always runs it down. Unreachable while nothing sets the penalty.
- **`Lock` and `gate_penalty` are not in the replay hash walks**, to keep every pinned hash; they are
  in the netplay snapshot.
- The latched brake lever is not written back to `Controls` (the original overwrites `+0x640` in place);
  any later reader of the pad's brake rather than `Held` would see the unlatched one. In ride.rs only
  `held.*` is read after the limiter.

## Next

P188 when a race mode exists. Nothing else open here.
