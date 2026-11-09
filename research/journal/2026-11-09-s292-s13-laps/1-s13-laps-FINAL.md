# 1 — s13 laps (FINAL)

Task S292. 2026-11-09. Part 4 of the S13c split: the other places the score manager's handlers call the
start-gate crossing test.

## What I did

- **Scoped it by the code, not by the task's names.** The task names "practice" and "fast-lap attack"
  handlers, but `score_manager_tick` (`./kit fn 0x25fcd0`) switches on the manager's **+0x64 mode**,
  not on +0x60, which is the kind `LapRules` reads. S92's store scan found that only modes 1, 2 and 3 are
  ever stored: 1 is the machine race, 2 supercross and hit fast lap (S293's handler), and 3 freeride and
  the stunt competition. That leaves these 12 callers of the crossing test:
  - `score_lap_line` (S13c) and the race manager's start-gate watch (S13a), both already ported.
  - The two inside mode 2's handler, which are S294's line lists.
  - **`score_gate_line`** (`./kit fn 0x2611c0`), mode 1's lap line. It is also called directly by three
    other activities (0x393420, 0x3a1d80, 0x3e3150) and from code at 0x3560a0. **Ported.**
  - **Mode 1's zone list** (inside `./kit fn 0x2640c0`). **Ported** as `race::zone_crossings`.
  - **Mode 3's zone list** (`./kit fn 0x268d00`). It is the same list, but it runs after the handler
    copies the card's place into the tick before's, so it never crosses anything. **Proved dead.**
  - Modes 4, 5 and 6 (`0x265040` ×2, `0x265c70`, `0x266750`). These modes are never stored, so the
    code is dead and not ported.
  - The vtable-only caller `./kit fn 0x285090`. It is an AI driver's steering: beside the card's next
    gate, it latches a turn-back flag and steers back along the path. That is AI work, not lap
    bookkeeping, so it is filed as **S292b**.
- `crates/game-sim/src/race.rs`:
  - New: `LapRules::gate_line`, its private `gate_run_over`, `race::zone_crossings` and `race_flags::ZONES`.
  - `LapCard` gains `gate`, `gates_this_lap` and `gates_passed` (+0x80, +0x84, +0x38).
  - `LineNotice` gains `Gate`.
- `research/oracle.txt`: new S292 block naming `score_gate_line`, `score_machine_race_tick`,
  `score_machine_zones` and `score_machine_zones_end`, with notes on mode 3's zones and the AI caller.
- Follow-ups: **S292a** for the rest of the machine race's per-card handler, **S292b** for the AI steering.
- opentennis has no laps or gates, so nothing was taken from it.

## What the original does

`score_gate_line(mgr, card)` returns 0 when there is no card, no gate (+0x80), or the clock ran
backwards, and 1 otherwise. The course is a chain of gates: the first at mgr+0x70, each gate's next at
its +0x90.

- **Through a gate:**
  - +0x24 grows by the distance from this gate's centre to the next one's (to the first one's after
    the last gate). The distance is `vsub.xyz`, then mula/madda/madd and `sqrt.s`.
  - The card moves on to the next gate, and +0x84 and +0x38 count up.
  - **Past the last gate a lap is counted:**
    - The card waits at the first gate again and +0x84 goes back to 0.
    - It counts much like the lap line. The differences: no 0x10 / 0x10000 / 0x20000 flag tests, and
      the best-lap test is `lap < best || best <= 0` (the line's is `best == 0`).
    - The best lap is told to the owner, the card's chain, and the manager's listener **twice**: this
      time it is the listener loop that starts at the listener itself.
    - A ghost race's run-over differs from the line's: the card's laps-behind become its lap length
      (not 0), the card keeps its lap time, the ghost takes the *card's* lap length as its laps behind
      and is not moved onto the line, and the ghost counts once the runs are in, with no best-time test.
  - **Through any other gate,** a kind-4 card clears 0x100.
- **Beside a gate:** the listeners are told, nothing else.
- **Every call, whatever the step did:** a card with its laps, in a race kind other than 2 or 3, is
  marked out again and the listeners hear it again. The one exception is the backwards-clock return.

The machine-race handler only runs the gate line when the card's +0x70 x (the last place's x) is
≥ 0: `c.olt.s $f0, $f24` with $f24 = 0. That part is S292a's, and is not explained.

The zone list (mgr flag 0x200, list +0x78, next +0x90): each live zone (flag 1) is tested on the
card's step. Through tells owner/chain vt+0x2c and listener +0x24; beside tells +0x30 and +0x28, each
passing the zone. Nothing is kept.

## How it was proved

`./kit check -- -p game-sim --test race -- gate_line zone`, all bit-exact. Three tests:

- **18, the gate line.** The original function on 20,000 generated cases: chains of 1–5 gates, cards at
  any gate (mostly the last) or none, every kind 0–5 (a third of them ghost races), ghost cards, lap
  records and pools, laps equal to the best, and stray float words.
  - Compared: the answer, every listener call in order, every word of the card and the ghost's card
    including the gate words, the manager's flags and the free records.
  - It asserts every notice the line makes comes up more than 100 times, and more than 1,000 laps.
  - Mutants that turned it red: `best <= 0` → `best == 0`, the run-over's laps-behind set to 0, a
    best-time test on the ghost-ready, the next-gate distance taken to the first gate, and the ghost's
    laps-behind taken from its own lap length. Dropping beside from the zones turned test 19 red.
  - One mutant survives: reading the race clock late instead of first. It is equivalent in the port,
    because nothing writes the clock before it is used.
- **19, the zone list.** That stretch of the machine-race handler, entered at its first pc with $16/$18
  set and stopped at the pc after it, on 20,000 zone lists about a step (some zones shut, the list off
  or empty). Compared against `zone_crossings` on every call and the zone each names. Asserts more
  than 2,000 throughs and more than 2,000 besides.
- **20, the stunt mode's zones.** The whole mode-3 handler on 5,000 steps. No zone call is ever made,
  even though in more than 1,000 cases the step it was handed would have crossed a zone.

## Not verified / not 1:1

- **No live capture.** Everything is a function oracle on generated gates. I didn't find which
  activities run a machine race on which tracks, and none has been captured.
- **The machine-race handler is not ported** beyond the line and the zones (S292a), so
  `LapRules::gate_line` and `zone_crossings` have no caller in the Sim. That includes the "last x ≥ 0"
  condition on the line, which is unexplained.
- **Which activities run the gate line directly** (the three direct callers and the code at 0x3560a0)
  was not read. Their use of the answer and of the card is unknown.
- **The gate chain is modelled as a list in order.** A card pointing at a gate outside the chain, or a
  chain with a loop, is not represented.
- **Listener calls are named, not ported** (as for the lap line). Any simulation effect a listener has
  is missing.
- **The race kind names** (`race_kind`) are still the S13c guess, which C31 showed wrong (+0x60 = 1 in
  a single race). They were not renamed here, to keep the merge with S294 clean. The gate line's logic
  depends only on the numbers.
- **Modes 4, 5 and 6's gate tests are not ported.** They are dead code: S92's scan found no store of
  those modes.
- **The AI caller is not ported** (S292b).
- **The ghost card being the card itself** is not generated (as for the lap line).

## Next

S292a (the machine race per card), S292b (the AI steering) and S294 (mode 2's line lists and finish).
