# 1 — s13 laps (PART)

Task S13c. 2026-11-09.

## What I did

- **Scoped it.** On sx01 the score manager runs race kind 2 (manager `+0x64` = 2, `+0x60` = 2, 5 laps,
  flags 0x5e80, read off S196's RAM image). Kind 2's handler (`./kit fn 0x00262aa0`, 5.4 KB, named
  `score_race_tick`) does all of a race's lap, checkpoint, off-track and finish work. It leans on the
  track-path queries (0x0026c010, 0x0026e0c0, 0x0026e630 and others, several KB between them), and it
  needs a live lap to hold against. That is many sessions of work. This session ported and oracled the
  piece everything else goes through: the score card's **lap line** (`./kit fn 0x00260820`,
  `score_lap_line`) and its lap-record helper (`./kit fn 0x00260710`, `score_keep_lap`). The rest is
  filed as S290–S292 (handler per card, line lists and finish, the other race kinds) and C31 (the
  captured sx01 lap).
- `crates/game-sim/src/race.rs`: `LapCard` (the card's lap words, by offset), `Course` (the line, its
  path spot, the lap length), `LapRules` (the manager's words), `Spot`, `LapRecord`, `LineNotice` (what
  the line tells listeners), and the flag/kind modules `card_flags`, `race_flags`, `race_kind`.
  `LapRules::line` is the line test; `keep_lap` and `run_over` (the ghost race's lap) are private.
- `research/oracle.txt`: named `score_lap_line`, `score_keep_lap`, `score_race_tick`, each with what it
  does.
- `crates/game-sim/tests/race.rs`: proof 6, `the_lap_line_matches_the_score_cards_line_test`.
- Nothing taken from opentennis. It has no laps.

## What the original does

`score_lap_line(mgr, card)` → 0/1. The card's flags are a 64-bit word (`ld`/`sd` at `+8`).

1. Card flag 0x10 → 1, nothing touched.
2. `start_gate_crossed(track+0x5c, card+0x60, card+0x70)`: the newest point first, the same order as the
   start-gate watch. 2 (beside) → the owner's vt+0x28, the card chain's vt+0x28 and the manager
   listener's vt+0x20 are told, then return 0. Any other answer except 1 → 0.
3. Through, card flag 0x10000 (behind the line): the flag is cleared. In kind 3 the listener's vt+0x4c is
   told. Then clear 0x20000 and return 1. No lap is counted.
4. Through otherwise: tell vt+0x24 / +0x24 / +0x1c. Then laps += 1, `+0xb8` = 0, and
   `lap = +0xb4 − +0x4c` (`sub.s`) is stored in `+0x48`. A negative lap stores 0 and returns 0, leaving
   the lap start and the 0x20000 flag alone.
5. Not 0x20000, and (best == 0 or lap < best) — the branch order, so a NaN-ish best is decided as the
   MIPS does: best = lap, and best number = kind 3 ? runs : laps − 1. Then the owner's vt+0x10 is called,
   followed by a loop that **starts at the owner again** (next `+0x1c`, not the card's chain), so the
   owner hears it twice. Then the listener's vt+0xc.
6. Lap start = race clock. Manager flag 0x400 → `score_keep_lap`: lap index = kind 3 ? runs+1 : laps.
   Under the pool size (`mgr+0x4c`), it pops the free list `mgr+0x58` (memset 0x18). Otherwise it detaches
   the card's oldest record. The record {lap time, lap start, card `+0x108`, lap, place, next} is
   prepended to `+0x1bc`.
7. Kind not 2 or 3, and `mgr+0x94 <= laps`: card |= 1, mgr |= 0x10, tell vt+0x18 / +0x18 / +0x14.
   **A single race (kind 2) never finishes here.** Something else marks its cards done (S291).
8. Kind 3 with the owner's `+8` card (the ghost's):
   - The rider's run starts over (clocks, progress, laps zeroed, runs += 1) and listener vt+0x54 is told.
   - The ghost takes the best if it is quicker (listener +0x58).
   - The ghost's card is reset onto the line: its spot is the line's `+0x94..` with along + 1e-5 (0x3727c5ac), and its lap length and distance to the line are track `+0x54`.
   - Runs ≥ `mgr+0xf0` with a ghost best: ghost |= 2, listener +0x50.
9. Kind 4 instead clears card 0x100.
10. memset(card+0xe4, 0, mgr+0x88·4) (the per-line words), tell vt+0xc / +0xc / +8, clear 0x20000,
    return 1.

The manager's `+0x60` numbers line up with the event's activity types (1 Practice, 2 SingleRace, 3
GhostRace, 4 FastLapAttack). The port names them that way. That is inferred from kind 3's ghost card and
sx01's 2; nothing that writes `+0x60` was read.

## How it was proved

`./kit check -- -p game-sim --test race -- lap_line`: the original `score_lap_line` run in the
interpreter over **20,000 generated cards**, against `LapRules::line`.

- **Inputs varied:**
  - Any line, usually a clean standing gate so that most steps cross, sometimes any gate the crossing test's generator makes.
  - Card flags drawn bit by bit over every bit read, with junk in the rest, including the upper 32.
  - Every float word with stray patterns: denormals, Inf/NaN, ±0.
  - Negative laps; a lap exactly equal to the best; a ghost best equal to the card's or to the lap.
  - Kinds 0–5 (a third of them ghost races); with or without an owner, a chain, a ghost card and line words.
  - Lap records on or off, pool sizes 0–11, 0–3 free and 0–3 kept.
- **Compared:** the answer, every listener call in order as (who, slot), every word of the card and of the ghost's card bit for bit (the lap records walked off the list), the manager's flags and the free list's length.
- **Coverage, from a temporary probe:**
  - 5,066 lines counted.
  - Notices: beside 4,436, through 3,614, started 406, best lap 1,129, finished 646, run over 649, ghost best 283, ghost ready 439, lap 2,188.
  - The test asserts each notice > 100 and > 2,000 counted.
- **Mutants:** each of these turns it red (the first two only once the equal-lap and equal-ghost cases were added: they survived the first generator):
  - `<` → `<=` on the best lap and on the ghost's best.
  - The 1e-5 nudge set to 0.
  - The best-lap number without its −1.
  - The negative-lap test moved to −1.
  - `>=` → `>` on the ghost's runs.
  - The line words not cleared.
  - The zero test without the FPU's denormal read.

## Not verified / not 1:1

- **Not held against a captured sx01 lap.** Nothing in the kit can ride an sx01 race lap yet: there is no pad (uinput is root-only), and a race needs a `.p2m2` tape through the menus and round the track. Filed as C31. The proof is the function oracle alone.
- **The race handler is not ported**, so `LapRules::line` has no caller in the Sim: the path queries, on-track word, off-track clock and countdown, time limit, checkpoint and line lists, the finish countdown, ranking, the white and checkered flags. Filed as S290 (per card) and S291 (lists and finish).
- **Who finishes a single race** is unknown. The lap line never marks a kind-2 card done; S291.
- **The other callers of the crossing test** are not read: kinds 1, 4, 5 and 6's handlers, the kind-3 handler's own line, the kind-1 helper and one vtable-only function. Filed as S292.
- **Listener calls are presentation, and the port only names them.** `LineNotice` lists them in order; the oracle checks every call (who, slot) through hooks. What each listener does (the HUD, the announcer, the owner's reaction to finishing, which may hand the bike to the AI) is not read. Any simulation effect a listener has is missing.
- **The owner hears a best lap twice** in the original (the loop restarts at the owner). The test models this; there is no owner to hear it in the port.
- **Null cards and owners** are not represented: the original returns 0 for a null card. The port takes references.
- **Past the record pool with no kept records**, the original writes the record through its caller's stale `$s1` (the manager) and links the manager into the card's list. No race reaches it: a card's own records fill before the pool runs out. The port keeps nothing, and the oracle gives such cards a record.
- **Not identified:** card `+0x108` (copied into each record) and the meaning of card flags 0x10 and 0x100. They are carried as words with neutral names.
- **The race kinds' names** come from the event's activity-type numbering, which is inferred (see above).
- **The ghost's card is a separate card.** The oracle never makes the owner's ghost card the card itself.

## Next

S290 first: the race handler's per-card path work, which gives the lap line its caller. Then S291, S292,
and C31 to hold the whole thing against a live lap. S13c waits on those (blocked `--after`) and is done
when the line and the handler are held against C31's capture.
