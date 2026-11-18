# 1 — rider stats page

Task P363. 2026-11-18.

## What I did

- A new **Statistics** screen (`--screen stats`), opened by **Square on the Race pick**.
  - Player 0's record (the one rider with a profile, as P335/P358), named by the settings' rider name.
  - Two panels side by side, after the original's statistics page:
    - Global Statistics: Gameplay Hours, Miles (or Kilometres) Ridden, Preloads, Clutches, Wrecks.
    - Racing Statistics: Races, Wins, Win Percentage, Hours Racing.
  - Left/Right (L1/R1) swap miles and kilometres. Circle or Cross go back.
- `menu.rs`: `Screen::Stats`, `Menu::stats_metric`, the press handling, and `stats_rows` /
  `hours_text` (the page's words, testable without drawing).
- `menu_screens.rs`: `Pen::stats` draws it. The Race pick names Square's Statistics in its hints
  only when the touch pad is down: P369's repair dropped a fourth hint there because it shrank the
  hint tab too far on a phone with the pad up.
- Why Square on the Race pick: the records are race records, the shop already hangs off the same
  page (Triangle), and a new Main Menu row would move every main-menu index for a low task.
- Nothing in `../opentennis` shows profile statistics; nothing taken.

## Format choices (ours to design)

- Times: `3 h 05 min`, or `12 min` under an hour (whole minutes, cut down).
- Distance: feet / 5280 miles, or feet × 0.3048 / 1000 km, one decimal.
- Win percentage: the record's own word, rounded to a whole number. With no races it shows `-`
  (the record's word is 100 then, as the original's rate gives when wins equal races).

## How it was proved

- `tests/menu.rs`:
  - `the_race_picks_square_opens_the_statistics`: Square opens it, Left/Right swap units, Circle back.
  - `the_statistics_rows_read_the_riders_record`: every row's words for a made record, both units,
    before and after races.
  - The existing every-screen tests (reachability walk, touch-pad hint room, title room) now cover
    the new screen too. `./kit check -- -p openmx --test menu` passed, 308 tests.
- Looked at shots by eye: desktop 1280x720 and phone 2340x1080 of the page, and the Race pick's hints.

## Not verified / not 1:1

- **Not the original's layout or words.** Menus are ours to design; the original's statistics page
  (0x003b7030 in research/oracle.txt) was used only for the list of statistics. I did not open it in
  PCSX2 to compare.
- **Gameplay time is play seconds in 16 bits** (the profile's word, P358), so it wraps at about
  18 hours. Shown as kept; not widened.
- **The units are not saved.** The page starts in miles every time; there is no front-end units
  setting yet (the results board is called with feet too).
- **Only player 0's record shows.** Split-screen seats have no profiles of their own yet (P358).
- **No phone check with the touch pad up on a real device.** Only the headless shots and the
  touch-room tests saw it.
