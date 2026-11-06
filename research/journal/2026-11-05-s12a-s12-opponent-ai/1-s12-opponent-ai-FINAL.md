# 1 — s12 opponent ai (FINAL)

Task S12a (the first piece of S12). 2026-11-05.

## What I did

- **Split S12.** "Opponent AI and the race activities" is several sessions of work: the AI update alone
  is `./kit fn 0x00386d30` (5 KB) on top of `./kit fn 0x002858d0` (4 KB), before the route data, the
  avoidance or the bikes themselves. Filed, in order:
  - **S12a** the roster draw (this entry),
  - **S12b** the route controller (throttle/brake/steer from the AI path, skill's effect),
  - **S12c** the feeler fan and per-rider avoidance (the 0x80/0x40 gate S14 found),
  - **S12d** AI tricks (the chooser is oracled already in `tests/trick.rs`; wire it through
    `ai_gesture_gate`),
  - **S12e** AI riders in the Sim, proved against a free09 4-racer capture,
  - **S12f** (low) the career/championship rosters (event+0x116 set), after S13.
  S12 itself stays open: it is done when S12a–e are.
- **Ported the roster** as `crates/game-sim/src/ai.rs`: `skill_range(difficulty)` and
  `spawn_roster(&mut Rng, RosterIn) -> Roster`.
- **Proved it** in `crates/game-sim/tests/ai.rs` by running the original's whole spawn in the
  interpreter.
- Named `player_construct` in `research/oracle.txt`.
- opentennis has an `ai.rs`, but it is tennis shot AI. There was nothing to take for a roster draw.

## What the original does

`./kit fn 0x00207b20` (`spawn_racers`), on the default path (event+0x116 clear):

1. **AllAI** (`all_ai` non-zero): humans is forced to 1, then two draws go into the caller's object:
   +0xe8 = `index(NumberOfRacers − 1) + 1` and +0xf0 = `8 + 5·unit()`. What reads them was not traced.
2. **Ghost race** (event+4 == 3): a kind-3 player numbered 1, then one human numbered 0 (human id
   table entry 0). No draws and no AI riders.
3. Otherwise the humans, kind 1, numbered 0…, then:
   - **the skill range** from event+0x117: 100 and up → [1, 1]. Otherwise
     a = 4 when d ≥ 90, else `trunc(0.5 + (4 + 6·(1 − (d − 70)/20)))`;
     b = 7 when d < 80, else `trunc(0.5 + (1 + 6·(1 − (d − 80)/20)))`; the range is (min, max) of the two.
     (`./kit dis 0x00207c54 0x00207d48` for the instruction order. Every op is single precision.)
   - **the identities**: for each of `NumberOfRacers − humans` AI riders, draw
     `index(count − 22) + 22` (count from the rider database, `./kit fn 0x003712a0`), and draw again
     until it differs from every one drawn before. They go into a 20-int stack array.
   - **the AI players**, kind 2, numbered on from the humans: +0x6b8 = the database entry
     (`./kit fn 0x003712b0`), +0x6bc = `low + index(high − low + 1)`. One draw per rider, *after*
     all the identity draws.
   Each draw is guarded by the lazy clock `srand` (`rand_seeded`). The port takes the caller's generator.

The draws use exactly `Rng::index`'s instruction shape: `2⁻³¹ × float(rand)` first, then
`× float(count)`, then `cvt.w.s`. So the port uses `Rng::index` and `Rng::unit` and nothing new.

## How it was proved

**Function oracle over 10,000 generated spawns**, `crates/game-sim/tests/ai.rs`. The original
`spawn_racers` runs whole in `ps2_oracle::Ee`. `operator_new` and `player_construct` are hooked (a
zeroed 0x800 buffer, and the constructor's six field writes). The two virtual calls go through a fake
vtable to hooked addresses that record the kind and the order players are added. The cases cover:
racers 0–12, humans 0–4, the difficulty either random 0–255 or one of the edges (0, 69, 70, 79, 80, 89,
90, 99, 100, 255), database sizes from just enough to tell the riders apart up to 59, AllAI 1 in 8,
ghost race 1 in 16, and a random 64-bit generator state. Each case compares every player (kind, number,
identity entry, skill), the AllAI pair bit for bit, and the generator state afterwards. One run reached
648 ghost races, 1162 AllAI spawns and 2301 grids of 8+ AI riders.

Two mutants both fail it: rounding with 0.25 instead of 0.5, and handing out the identities in reverse.
The unit test's hand-worked range for difficulty 79 was wrong at first (I wrote (5, 7) and the proved
port gives (7, 7)). It now checks 75 → (7, 9).

## Not verified / not 1:1

- **The event+0x116 branches are not ported** (the saved roster from `./kit fn 0x0036c0b0` and the
  championship table from `./kit fn 0x00369cb0`). I don't know where those objects come from yet. Filed
  as S12f, after S13.
- **The lazy wall-clock `srand`** inside the spawn is not in the port. The caller's generator is used
  as it stands, and the oracle runs with `rand_seeded` set. A port that starts a world with an unseeded
  generator would need that seed from its caller. That is a question for S12e and S13.
- **The impossible rosters panic instead of hanging.** With fewer distinct drawable identities than AI
  riders, the original re-draws for ever. With more than 20 AI riders, it writes past its stack array.
  The port asserts on both. The oracle never generates either.
- **What the AllAI pair is for** (caller +0xe8, +0xf0) was not traced. The port returns them as
  `AllAiDraw` because they are draws in the sequence. Its `racer`/`seconds` names are a guess.
- **Which identities the rider database actually holds** (its count on the disc, and what entries 0–21
  are) was not read from a RAM image. The oracle feeds generated counts. S12e needs the real count.
- **Humans' +0x6b4 human id** and the player objects' other fields are not part of the port. Only the
  dice are.
- **Not against a live capture.** The real spawn seeds from the clock, so a capture's roster can't be
  reproduced from its start. The oracle runs the original's own instructions over the whole input
  space instead.

## Next

S12b: find the AI's route data (the track .res waypoints the AI follows) and the control outputs of
`./kit fn 0x002858d0` / `./kit fn 0x00386d30`. List their branches against the PC exe first
(`./kit fn … -o pc`, MXVehicleAI.cpp).
