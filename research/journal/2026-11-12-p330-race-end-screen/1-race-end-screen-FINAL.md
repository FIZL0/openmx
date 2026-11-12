# 1 — race end screen (FINAL)

Task P330. 2026-11-12.

## What I did

- `crates/openmx/src/menu_results.rs` (mounted as `menu::results`): the results menu's take of the
  score manager's cards, ported — the per-card record (`ResultCard::take`), the list's sort (`sort`),
  the time text (`time_text`) and the eleven pages' cells (`Board::take`); the preload and clutch
  popup's words per seat (`tips_of`, from `RaceEnd::tips`); the Hall of Fame menu's marks
  (`FameMarks::mark`, from `RaceOver::fame`); the profile's race and win counts with the win
  percentage (`RiderRecord`), kept per seat in `records.txt` (`Records`, loaded and saved by the shell
  next to settings.txt).
- `menu_race.rs`: `Menu::race_ended(HandOver)` takes what the race hands over at its end (the
  cards, `race_end`'s answer, every `race_over` answer), counts each `RaceOver::win` in that seat's
  record and keeps the rest for the leave; `race_left` then builds the results from it
  (`Results::handed`). The results screen runs in stages: the tips (if any seat skipped the preload
  or clutch), the board (Left/Right and L1/R1 step the eleven pages round, as the original's paging
  does ±1 mod 11), the Hall of Fame (when a mark is waiting — the original's way out goes there first
  too), then the main menu. With no hand-over (every race today) it is the P327 stand-in as before.
- `menu_screens.rs`: draws the three stages. The board is the page's title, column heads and the
  cells exactly as `Board::take` wrote them. The tips and Hall of Fame wording is ours.
- `main.rs`: loads `records.txt` at start, saves it when a win changed it.
- The opentennis sibling has no race results or league menus; nothing was taken from it.

## What the original does

Read with `./kit dis` (all now named in research/oracle.txt's P330 block):

- `./kit fn race_results_take` builds a record list (count, up to ten records, the key it is sorted
  on, starting 0) with one record per card in the manager's list, copied by `race_results_card`:
  the seat item, the place (cvt), race time, max(0, gap), best lap, the statistics tracker's laps led
  and times in lead (cvt), the card's longest jump and hang time, then the tracker's passes, times
  passed, wrecks, preloads and clutches (cvt). The tracker is the item's interface slot, looked up by
  the "StatisticsTracker" id.
- Then for each of its eleven pages (keys 0, 3..12): sort on the page's word, set the page's row
  count to cards + 3, and per row snprintf into 25-byte cells: the row number, the name (a player's
  profile name, an AI's name, or — any other seat item kind — nothing: the cell stays as the page
  was made), and the value(s). Page 0 shows race time and gap as times; keys 3 and 7 show a time;
  key 6 a distance (feet, or `cvt(0.3048 × ft)` with the front end's metric word); the rest `%d`
  of cvt. Three blank rows follow. At the end it sorts on page 0's key again.
- Time text: `--:--.--` for 0 (c.eq), `99:99.99` unless t c.ole 5999.99, `-99:99.99` if t c.olt
  -5999.99, else cs = cvt.w.s(100t + 0.5) written `[-]m m:s s.c c` with cs/60000 as the first digit.
- `./kit fn race_results_sort`: skipped when the key is the list's key; keys 0–3 ascending, the rest
  descending; c.olt insertion sort from the second record, equal words keep their order; on key 3 a
  moving record's 0 best lap is 18000.0 while it moves and 18000.0 is written back as 0.
- `./kit fn fame_menu_mark`: seat 0 sets +0xb0 and clears +0xbc; any other seat sets +0xb4 (and
  rewrites +0xbc as 1 if it was non-zero); then the track and the waiting word.
- `./kit fn profile_stat_add` cases 9 (races) and 10 (wins): u16 += cvt(amount) & 0xffff, then the
  win percentage = 100.0 when equal, else 100 × wins / races (the 16-bit counts through cvt.s.w).
- The Hall of Fame tables' defaults are generated at boot (a rand-driven generator from per-track
  base tables), not stored: the s13d capture's tables are non-zero from the start.

## How it was proved

Function oracles in `crates/openmx/tests/race_results.rs`, all passing against the game image
(a probe confirmed the image and every name loaded, so none skipped):

- `race_results_sort` × 20,000 lists (0–10 records, every key from every key, zeros, 18000, ties,
  random bit patterns): order, every record word and the list's key, bit for bit.
- `race_results_take` × 6,000 card sets (1–10 cards; players, AI riders, neither; names 0–40
  characters; times at 0, ±0, either side of ±5999.99 by a few ulps, random bits, negatives;
  metric on and off): every cell of all eleven pages including the three blank rows, each page's
  row count, the records left after the sorts. snprintf, strcpy, the allocator, the interface id,
  the AI's name and the tracker slot are hooked; the menu's code and its card copy run.
- `profile_stat_add` stats 9 and 10 × 20,000 (random counts, amounts and slots): both counts and
  the win percentage's bits.
- `fame_menu_mark` × 5,000: all five words.
- Plus the popup's words (`tips_of`), the records file round trip, and `tests/menu.rs`'
  `a_race_end_shows_its_tips_board_and_hall_of_fame_and_counts_the_win` walking the screen's
  stages by pad reports.

Found on the way: for a seat item that is neither a player nor an AI, the original never writes
the name cell (the row number it just formatted stays in its scratch buffer only), so the cell is
whatever the page held — empty for a page just made. The port shows it empty.

## Not verified / not 1:1

- **Nothing feeds it yet.** The Sim does not run the race (S301 open): no score manager cards,
  tracker stats or `race_end` / `race_over` answers reach the shell, so `Menu::race_ended` has no
  caller and every real race still ends on the P327 stand-in. The hand-over path is proved only by
  `tests/menu.rs`' walk with made-up cards. Follow-up filed.
- **The statistics tracker** (laps led, times in lead, passes, times passed, wrecks, preloads,
  clutches) and the card's longest jump / hang time words are not in game_sim; `CardStats` is the
  slot they fill. Not traced who writes them.
- **Metric is always off**: the shell has no units setting, so distances show in feet. The metric
  branch itself is oracled.
- **The Hall of Fame itself is not ported**: no name entry, no table insertion, no default table
  generator. The screen only says a seat made the Hall of Fame. Follow-up filed.
- **Races are never counted**: `profile_stat_add` stat 9 is oracled, but who calls it at a race's
  end was not traced (a stats commit with many stats), so only wins are counted and the win
  percentage is the FPU's 100 / 0 (its largest float) after any win. Follow-up filed.
- **The tip popup's own display** (`tip_popup_arm`, its pictures and timing) is not read; our text
  says the same thing in our words, not the original's strings.
- **The original's buttons are not 1:1**: its Restart / Quit (single race) and Next (league) choices
  are not offered; Cross goes on to the next stage and then the main menu. The three blank rows are
  not drawn.
- **No live check**: no PCSX2 capture of the results menu's pages was compared; the proof is the
  function oracle with hooked snprintf / strcpy / allocator.

## Next

S318 wires the hand-over once S301 runs the race; S319 fills the tracker words; P331 is the Hall of
Fame itself; P332 counts races; P333 is the original's buttons and popup.
