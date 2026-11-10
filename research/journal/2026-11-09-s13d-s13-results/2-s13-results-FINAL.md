# 2 — s13 results (FINAL)

Task S13d. 2026-11-10.

## What I did

- **Ported the supercross activity's two answers at a race's end** to `crates/game-sim/src/race.rs`
  (section "the race's results (S13d)"):
  - `Supercross::race_over`: the answer to a card's finish at the line. It covers the winner's
    fireworks, the podium camera flashes, a player's win counted in their profile, and the Hall of Fame
    entries a player's best lap and race time earn.
  - `Supercross::race_end`: the answer to the race's Over notice, the hand-back to the front end. It
    hands the race results menu the score manager, writes the preload and clutch tip popup's words per
    seat, picks the outro scene, and cues it.
  - `HallOfFame` with `fame_table`: the front end's "does this time earn a place" over its 11 tables.
  - The types they take and give: `ResultEvent` (with `alt_outro`), `ResultSeat`, `PodiumFlashes`,
    `FlashLevels`, `FameEntry`, `RaceOver`, `EndSeat`, `EndTip`, `RaceEnd`.
- **The rest of the results was already ported**: places (`LapRules::rank`), finish times and best
  lap at the line (`LapRules::line`), the made-up finishes (`LapRules::clocks`), and the tie draw and
  over notice (`LapRules::close`). S294 and S13c landed them. This session holds them live for the first
  time.
- **Captured an sx01 race finish** (`research/s13d/run.sh`, `research/s13d/finish.py`, kept at
  `context/capture/s13d/sx01-finish.jsonl`):
  - a 4-rider, 1-lap single race from C31's booted start (`research/c31/boot.py`), the human idle at the
    gate;
  - once an AI rider had finished, the human's card was poked done (card +8 |= 1), and the race ended
    on the next tick;
  - every tick: the manager, all four cards, the activity, the podium flash object, the stats; before
    and after it, the Hall of Fame tables, event, front end and profiles. The track's flash levels are
    a `world` line appended from the same session; finish.py now writes them in each snapshot.
- Corrected part 1's reading: **the fireworks are not called at the race's end.** `race_over_card`
  sits at the manager's listener slot +0x14 (the lap line's Finished), not +0x18 (Over). The capture
  confirms it: the flashes answer the winner's crossing at frame 2538 and do not move at the over tick
  (2567). The Over slot is `race_end` (0x002fc970), which part 1 had not found.
- oracle.txt: the S13d block (`race_over_card`, `race_fireworks`, `hall_of_fame_beats`,
  `results_track_list/find`, `fame_menu_mark`, `profile_stat_add`, `race_end`,
  `race_results_take`, `tip_popup_arm`), the listener's vtable, and which slot is which.
- Filed: P329 (results menu, tip popup, Hall of Fame marks on screen), V127 (fireworks and podium
  flashes drawn), S305 (low: S13c's hold steps cards in the wrong list order). Noted on S301: wire both
  answers in.
- Nothing taken from opentennis; it has no race results.

## What the original does

Recipes: `./kit fn 0x002fc410` (race_over_card), `./kit fn 0x002fc970` (race_end),
`./kit fn 0x00377bb0` (hall_of_fame_beats), `./kit dis 0x002fc970 0x002fcce8`.

- **The manager's listener** is the activity +0x160 (its vtable at +0x1c, 0x00458690; the thunks
  subtract 0x160). Slot +0x14 → `race_over_card` (a card's Finished, from the lap line). Slot +0x18 →
  `race_end` (the Over notice, once, from the handler's close).
- **race_over_card (activity, mgr, card)**:
  - Finds the card's interface item: the activity's list (+0x14), each item's vt+0xa4 cast to
    "RaceStatusVehicleInterface", −0x24 holds the card. If none matches, it takes the last item.
  - In a single race (event +4 == 2), place 1 starts the fireworks (`race_fireworks`, presentation).
  - With the flash object (+0x414) and place < 4:
    - length (+0x64) = max(world +0x538, length);
    - half (+0x6c) = 0.5 × length;
    - rate (+0x68) = (target − from (+0x5c)) / half;
    - the target is world +0x53c for a player's card in first, else +0x544.
  - **A player's card only (+8 & 4).**
    - The track id comes from the league's track list (−1 if not found).
    - Mode 0 + single race + place 1 + a profile on the seat counts a win (`profile_stat_add` stat 10).
    - In a single race with the activity's +0x4d0[seat] set, the best lap is tried on table 0 (league
      0) or 4, and the race time on +1/+2/+3 for 3/5/10 laps. Each one that beats an entry marks the
      HOF menu (`fame_menu_mark`).
- **hall_of_fame_beats (time, tables, table, track, class)**: 1 if the time beats any of the five
  entries. Supercross tables compare whole seconds (`cvt.w.s` then back); the scores table compares
  `!(t <= kept)`; the counts table compares integers. Table starts are in oracle.txt.
- **race_end (activity)**, single race:
  - Hands the "RaceResultsMenu" the score manager.
  - Walks the list: a card whose seat item (owner +0x24 → +0x20) has +0x34 == 1 is a player's. Its
    "StatisticsTracker" (item vt at +0x48, slot +0x40) gives +0x38 / +0x3c. The best place is the
    players' minimum (from 4). A player not in first sets the "PreloadAndClutchTipPopup" words +0xc0[seat] =
    (+0x38 == 0) and +0xc8[seat] = (+0x3c == 0).
  - Scene +0x470: 2/4/6 for best place 1/2/other, 1/3/5 when event byte +0x116 is set. Then the popup
    is armed.
  - Any event, with a scene and +0x474 clear: every interface item's MotocrossRider (vt+0xa0, −0x400)
    has its +0xa74 told (vt+0x9c), and the scene's cue (+0x44c[scene]) is started. A cue that answers
    0 sets +0x474 = 1 and +0x410 = 7.0. If app+0x570 is set, the music fades to 0.75.
  - In the capture the cue had something to play. The activity's own tick (`race::Race::tick`,
    already ported) armed it one tick later, with +0x410 = 6.46, then counted it down to 0 at 2762.
- **Live, at the over tick** (frame 2567, manager flags 0x5e11 → 0x7e13):
  - the human's card done and no player racing, so OVER;
  - card 2 still out: its finish made up as 74.5229 + 3.8357 = 78.3587, its lap time 3.8357 (gap
    plus lap start, over the one lap left), best the same, laps 1, OUT;
  - the tie draw: the human's 75.4893, behind card 2 in the list, pushed to 78.3587 + `rand()`·2⁻³¹ =
    79.2767. It is the tick's first draw, and four more follow before the row's state;
  - the ranking keeps 3, 1, 2, 0 (places 1–4);
  - `race_end`: scene 6 (the human last, event +0x116 = 0), not armed.

## How it was proved

All in `crates/game-sim/tests/race.rs` (header items 29–32). `./kit check -- -p game-sim --test race`:

- **`the_hall_of_fame_test_matches_the_originals`**: `hall_of_fame_beats` in the interpreter on 20,000
  tables, tracks, classes and times: whole seconds and a ulp either side, 0, negatives, integer
  entries. Over 150 of each answer per table.
- **`the_race_over_matches_the_supercross_activitys_answer`**: `race_over_card` on 20,000 cases:
  - events across leagues, activities, classes, laps and modes;
  - tracks found or not (−1);
  - player and AI cards at any place;
  - 1–3 interface items, the card's among them or not;
  - seats with and without records and profiles;
  - flash objects present or not.

  Compared: every call (fireworks with its item, the stat with its slot and 1.0, each HOF mark with
  seat and track) and the flash words, as bits. Mutants caught: whole seconds, the player-in-first
  level, the win's mode gate, the 3/5-lap table swap. One survived, the scores `!le` written as
  `lt(kept, t)`, and it is equivalent.
- **`the_race_end_matches_the_supercross_activitys_answer`**: `race_end` on 20,000 races:
  - 1–6 cards at any place (ties, none first, 0 and below);
  - players and AI, seats 0–1, trackers with and without use;
  - single race or not, the +0x116 byte;
  - the scene already picked or armed or not;
  - 1–3 items, cue ending or not, fade on or off.

  Compared: every call in order with its arguments (results menu + manager, popup arm, each rider
  told, the cue's scene, the fade's 0.75), the popup's four words, and +0x470/+0x474/+0x410. Five
  mutants, all caught: the scene table, `place != 1` vs `> 1`, the clutch word, the armed gate, the
  best place clamped.
- **`the_results_hold_s13ds_live_sx01_finish`** on the capture:
  - the winner's finish at the line (frame 2538, card 3, AI) through `race_over`: fireworks, no seat;
    the flashes' half 1.0 and rate 0.29 bit for bit, and the length 2.0 one tick down;
  - the over tick leaving the flash half and rate alone;
  - the over tick through `check_over` → `clocks` → `close` on the tick before's cards in list order,
    with the generator searched forward from the tick before's state (draw found 0 steps on). Matched
    on every card's id, OUT, place, laps, race time, lap time and best lap in list order, the
    manager's OVER/OVER_TOLD, and the tick's end state reachable from the draw;
  - `race_end`: scene 6, armed 0, as the activity's +0x470/+0x474.

## Not verified / not 1:1

- **The finish was poked, not ridden.** The human never crossed the line, so a player's
  `race_over` (the win, the Hall of Fame entries, the player-in-first flash level, the HOF menu
  lookup) is proved by the oracle only. A player finish needs a ridden lap (a generated `.p2m2` tape;
  no pad here).
- **The second AI finish (card 1) is not held.** The recorder lost frames 2539–2564 (one gap,
  "skipped 0"), and card 1 finished inside it. Only one finish at the line is held live.
- **The track id −1 and out-of-region indexes**: the port's `HallOfFame::beats` answers false. The
  original reads outside its tables (−1 reads the 0x50 bytes before the first). The oracle filled those
  with words no time beats, and skipped out-of-region cases.
- **A player's card with no matching interface item**: the original takes the last item's seat. The
  port takes the seat from the caller (`ResultSeat`); a caller with no item passes `None`, and the
  original's fallback is not modelled.
- **The seat item's +0x34 == 1 test (player for `race_end`)** is the caller's to make (`EndSeat`). The
  live hold used the card's PLAYER flag instead, because the seat item is not captured. The tip
  popup's words and the statistics tracker (+0x38/+0x3c, read here as preload and clutch use from the
  popup's name) were not captured, so the popup is oracle-proved only. Seats are 0 and 1: seat 2 would
  write seat 0's clutch word, as the original does, and the oracle did not try it.
- **Presentation not ported**, only the calls: the fireworks (`race_fireworks`), the riders told
  (vt+0x9c), the cue, the music fade, the results menu (`race_results_take`), the popup arm, and the
  HOF menu mark's words. Filed as P329 and V127.
- **The HOF insertion** (writing the new entry into the tables) is the front end's. The "after" snapshot
  shows the tables unchanged in this run (no player entry).
- **The 30 s countdown (manager +0x98/+0x9c) is not live.** It runs only after a player's line finish
  (flag 4) while others race. The poke skips it. Oracle (S294) only.
- **What sets the activity's +0x4d0 per seat** (whether a seat keeps records) was not traced. In the
  capture it is 0 for seat 0.
- **`alt_outro` (event +0x116)**: what it marks was not traced. It is 0 in the capture.
- **The over tick's path work** (S293) is not run in the hold. The cards' progress is taken from the
  captured row. Only the clocks, close and ranking run.
- **Neither answer runs in the Sim yet.** Wiring them in is noted on S301.
- **`0x00266d50`** (68 bytes, after every mode but 5, which part 1 guessed was the places update) was
  not read. The places as held come from the ranking, which matches the capture.

## Next

S301 (wire the race step and both answers), P329 and V127 (on screen), S305 (the C31 hold's order).
