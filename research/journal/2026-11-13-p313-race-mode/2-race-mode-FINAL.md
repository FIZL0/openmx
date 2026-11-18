# 2 — race mode (FINAL)

Task P313. 2026-11-18.

## What I did

- Everything the reopened task waited on has landed: AI riders on the grid (P313a), drawn (V128), racing
  on the track's AI net (S331b, S331c), the gate drop on screen (P313b, P313c), two-seat races (P350), the
  crowd cheer (C23). This session checks the whole race in a release build and adds the missing test: a
  race with AI riders that laps all the way to its results.
- `--race-bot` (main.rs, the port's own, off by default): the race's first human rides on an AI brain, so
  a headless race laps to its end with nobody at the pad. `race_start::spawn_ai` takes a `bot` flag. With
  it, the Sim's roster has no humans, and `race_start::bot_rider` (the first AI identity, the top of the
  race's skill range) is spawned before the drawn roster's riders. The roster draw itself is unchanged.
  The line: `race: player 1 rides on a brain, skill N`.
- At the Over, the shell now builds the results board (`race_start::hand_over_seats`) with or without a
  front end, and prints it as the results screen's rows read: `race: results — 1. Player 1 76.56s, …`.
  The front end's hand-over is as before.
- Test `a_race_with_ai_riders_laps_to_its_results` (crates/openmx/tests/race_menu.rs): `--race --laps 1
  --racers 4 --race-bot`, 95 s of the steady clock on sx01. It checks three AI riders and the bot, every
  player's finish after the drop, the Over, a board of four placed 1 to 4 with rising race times (the
  human's card and three AI riders), and the outro's end.
- Took nothing from opentennis: it has no AI-driven player.

## What the original does

- AllAI does not put the player's bike under AI: the player's card stays at the gate (C31, memory
  race-laps-from-ai-riders). So the original has no counterpart to `--race-bot`. It is a test aid only.

## How it was proved

- Release build (`cargo build --release -p openmx`), sx01, `--race --laps 1 --racers 4`:
  - Without the bot (200 s): three AI riders in lanes 1, 3, 0 behind the closed gate. Drop at tick 97,
    drawn gate down at tick 101. The AI riders lap and finish (73.52 s, 73.92 s, 77.79 s). The race is
    never over, since the human sits on the grid, as in the original.
  - With the bot (150 s): all four finish (76.56 s to 82.28 s), Over, results board of four, outro over.
  - GPU shots (headless, release): AI riders drawn beside the human on the grid at frame 60. The four
    riders ride off the start together at frame 180. Looked at by eye. The shots are in this session's
    scratchpad only, not kept.
- `./kit check -- -p openmx --test race_menu`: 5 tests passed (the new one, and the four earlier ones:
  gate, free ride with no `race:` line, AI riders behind the gate, two-seat race).
- The results screen from the menu: `tests/menu.rs`'
  `race_from_the_main_menu_rides_its_setup_and_shows_results` and
  `a_race_end_shows_its_tips_board_and_hall_of_fame_and_counts_the_win` (unchanged).
- Free Ride's replay hashes: `--race-bot` changes nothing without `--race` and the flag. The gate's
  replay tests run unchanged.

## Not verified / not 1:1

- **The headless test does not go through the menu.** The board is printed from the same hand-over the
  front end gets, and the menu's own tests show that hand-over becoming the results screen. But no single
  run walks Main → Race → ride → Results: the front end needs a window.
- **The bot's race is not the original's.** A brain on player 0 draws on the race generator before the
  roster's brains, so the AI riders' times differ from a race without it. It is a test aid only.
- **The Over came right after the human's finish**, with every AI card given a time just after the
  human's (the over-told draws), though in the no-bot run they took 73–78 s for the same lap. This is the
  ported close as S13 proved it, but it was not compared against a live race end with AI riders.
- **No live race capture compared** for the whole race (grid, drop, laps, finish order). Each piece was
  proved by its own task. The wiring as a whole was checked only by riding the shell.
- **Not looked at in a window.** The release shots are headless GPU frames. A desktop window run, menu
  to results, was not done here (no keystrokes on this desktop).
- The earlier notes in `1-race-mode-PART.md` and P313a/b/c's journals still stand (AI riders' bike, their
  difficulty and seed, the results' empty trackers, the gate's pose).

## Next

Done.
