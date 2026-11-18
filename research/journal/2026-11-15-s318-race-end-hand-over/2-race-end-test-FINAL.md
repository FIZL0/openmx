# 2 — race end, ridden to the board (FINAL)

Task S318. 2026-11-18.

## What I did

- Re-read the shell's race end after S301 and S319 landed. The hand-over the task asks for is
  already in place:
  - the Sim's `Laps` feeds each card's tracker every frame (`Laps::bike` from the bike's preload
    timer, clutch lever and wreck entries; laps and place changes from the lap line);
  - `race_start::hand_over_seats` takes the cards in list order, each with its tracker words and
    its card's longest jump and hang time (S319 filled `CardStats` there);
  - `race_end` gets each player seat's real preloads and clutches;
  - every `race_over` answer goes in, the human's heard against the profile's Hall of Fame tables
    (P354); `main.rs` calls `Menu::race_ended` at the Over.
- So no product change in `main.rs` was needed. What was missing was the task's test.
- Added `a_race_ridden_to_its_over_ends_on_its_trackers_board` in `crates/openmx/tests/menu.rs`
  (mounts `race_start.rs` there):
  - two riders walked round sx01's paths (as game-sim's P313 race test walks them), three laps,
    the human faster;
  - each tick feeds the bikes' counts: the human pulls the clutch twice, never preloads; the
    computer's rider preloads three times and wrecks twice;
  - each `Finished` is answered with `Supercross::race_over`; the Over hands over to the menu;
  - checks: the trackers count 0/2/0 and 3/0/2, the human led laps (3), one finish answer (the
    computer's made-up finish is never answered), the win counted, no tip for the winner, and
    every tracker page's cell, plus the jump and hang words, is the rider's own.
- Took nothing from `../opentennis`: it has no race results board.

## How it was proved

- The tracker and jump words are S319's oracled port; `race_end`/`race_over` are oracled (S13d).
  This task only joins them, and the new test proves the join end to end on sx01's real paths.
- `./kit check -- -p openmx --test menu a_race_ridden` passes.

## Not verified / not 1:1

- Not ridden in the live shell to the board by eye: no pad here, and the test drives the same
  calls `main.rs` makes.
- The test needs the disc (sx01's race file). Without it, it skips with a line on stderr.
- AI riders are named `Rider N`, not by the AI's own name: the Sim keeps no AI rider names.
- The winner gets no preload/clutch tip, so the test's ride never shows the tip popup; the tip path
  itself is covered by P330's test with a hand-made hand-over, not by a ridden race.
- `times_in_lead`, `passes` and `times_passed` are 0 in this ride (the human leads from the start);
  the pages are checked against the tracker, but no ridden race here makes them non-zero.
