# 1 — race trick bank

Task C42. 2026-11-17.

## What I did

- A live sx01 single race from C31's start state (`context/states/c31-sx01-race-start.p2s`), with a tape
  that lands tricks, ridden to the finish, the results and Restart, and on through a second race and a
  second Restart. Scripts in `research/c42` (README.md lists them).
  - The tape (`make_pad.py`): gas held; a no-hander (Triangle + stick up, C8's) for 8 ticks every 45 ticks
    from tick 300 to 3300; then Cross lifted for 3 ticks every 30, so the results screen gets Cross
    presses (accept, event 0x36, Restart). No Triangle after 3300: on the results screen it is Quit.
  - The race was cut to one lap by poking the score manager's lap count (`mgr+0x94`, C31) after the
    state load, so the player's card finishes in ~6,500 ticks instead of three laps.
  - The watch (`c42.toml`) reads every tick: card 0's flags, laps, combo node count, combo seconds, bank
    (`+0xdc`) and combo points; the AI cards' banks; the manager's flags and laps; player 0's profile
    points (`(app+0x3f4)[0]` = 0x56fea0 in this session, `+0xc0`).
- `research/oracle.txt`: a C42 line under `results_score_commit`.
- My copy's ini had `NominalScalar = 0.05`. I ran at 1 (load ~25) and put 0.05 back.
- Nothing in `../opentennis` covers this (no profile points, no PCSX2 race capture).

## What the original does

Read off `context/capture/c42/trick1.csv` (16,000 rows, frames 1..16,510):

- **A race card banks the player's tricks.** Card 0's combo opens (node count 1, flags `0x...e6`) on a
  no-hander in the air and closes at the landing. 15 landings banked: `+0xdc` goes 0 → 1240 (tick 921)
  → 2730 → … → 31690 (tick 3167). Some combos closed without banking (node count back to 0, bank
  unchanged: wrecks or a press on the ground). Two combos held two nodes (ticks 1485, 2205).
- **The AI cards bank too**: riders 3 and 1 bank 2490 each (ticks 1613, 2160), as in C31's captures.
- **The bank holds through the finish and the results.** Card 0 crosses its one lap at tick 6496
  (manager flags gain 4, then 0x7e17 a tick later). From then to tick 6781 `+0xdc` stays 31690.
- **Restart pays it out.** At tick 6782 the profile's points go 0 → 31690: exactly `cvt.w.s(31690.0)`.
  In the same row every card's `+0xdc` reads 0 and the manager flags read the new race's 0x5e80, so the
  commit runs before the race's reset.
- **A zero bank adds nothing.** The second race (no tricks, card 0's bank 0 throughout) finished at
  13221; its Restart (13502) left the points at 31690.
- **Restart keeps the cut lap count.** The manager's laps stay 1 in the second race (not the event's 3).
- After the run, a direct read: frame 16531, points 31690.

## How it was proved

- A PINE recording of the live game (`./kit record`, `research/c42/c42.toml`), 16,000 rows, provenance in
  `context/capture/c42/trick1.csv.json`. 3 torn reads and 26 counter gaps (507 frames run past) skipped;
  frames 6770..6790, around the Restart, are all present.
- Points read directly before the run (frame 0: 0) and after it (frame 16531: 31690).
- This confirms what P364 ported (`Records::commit_score`, oracled over 10,000 races in
  `tests/race_results.rs`): a race card banks, and the commit adds its whole-number total.

## Not verified / not 1:1

- **The port was not ridden beside it.** No shell race was run with the same tape to compare its
  `records.txt` points against 31690. The port's bank comes from game_sim's score, not this card, and
  P364 already lists the P243 landing-lockout difference.
- **The race was cut to one lap by a poke** (manager lap count 3 → 1). A three-lap race was not ridden
  to the end; the commit does not read the lap count.
- **When the results screen appeared and which Cross press was the Restart is not seen.** The watch has
  no menu word; the tape presses Cross every 30 ticks. That the change came from Restart and not Quit's
  yes is inferred: no Triangle was pressed after tick 3300, and the race restarted (cards reset, a second
  race ran).
- **No Hall of Fame case.** The ~207 s lap (drop at 289, line at 6496) set no mark; a race with a mark waiting (where Restart commits
  nothing) was not ridden.
- **Only player 0 has a profile.** A split-screen race with two profiles was not captured.
- **The commit and the reset land in one row.** The order inside that tick (commit before reset) is from
  P364's reading, not seen mid-frame.

## Next

Done.
