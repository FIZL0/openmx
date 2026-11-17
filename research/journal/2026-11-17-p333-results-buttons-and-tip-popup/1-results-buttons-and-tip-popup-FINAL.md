# 1 — results buttons and tip popup (FINAL)

Task P333. 2026-11-17.

## What I did

- `menu_race.rs`:
  - The tip popup now has two pages (`TipPage`). The preload page comes first when any player
    missed the preload. Its OK (Cross) opens the clutch page when a player missed the clutch, else
    the board (`Results::tip_ok`). Circle and Triangle no longer close it.
  - Each page's title is the original's (`tip_title`): "Player 1 and Player 2 / Player 1 / Player 2"
    then "Preload Warning:" or "Clutch Warning:". The page's line is the original's English string
    (`PRELOAD_TIP`, `CLUTCH_TIP`).
  - The board's buttons are the original's button bar (`Results::buttons`). A single race has
    Triangle Quit and Cross Restart. A league has Cross Next (`Results::league`; always false here,
    as openmx has no league mode yet).
  - Quit opens the original's "Warning / Are you sure you want to quit?" (`Results::quitting`).
    Cross says yes; Circle or Triangle says no.
  - Restart and a confirmed Quit go to the Hall of Fame first when a mark is waiting, as the
    original's do. A league's Next skips it, as the original's goes straight to the standings
    (`Results::press`). The button is kept in `Results::after`.
  - The way out (`results_way_out`): Restart rides the same track, laps and riders again. The menu
    stack under the ride is the race setup, so a failed load lands there. Quit and Next go to the
    main menu.
- `menu_screens.rs`: draws the popup's title and line, Quit's confirm, the board's button bar, and
  "Restart" or "Main Menu" on the Hall of Fame pages' Cross by the button that led there.
- `tests/menu.rs`: the P330 walk now ends in Restart, which rides sx01 again.
  `the_results_menu_has_restart_and_quit_and_the_tips_come_page_by_page` covers both popup pages,
  the titles for two seats and for seat 1 alone, Quit's yes and no, and a league's Next.
- Addresses: `research/oracle.txt`'s new P333 block.
- opentennis has no results or popup menus. Nothing was taken from it.

## What the original does

- `./kit fn 0x0029abc0` (arm): sums the per-seat words into "any preload missed" and "any clutch
  missed". It writes both pages' text from the two English strings.
- `./kit fn 0x0029ad60` (open): a one-button bar, icon 0, STR_OK, event 0x36 (Cross). It plays the
  preload part if any preload was missed, else the clutch part, and marks itself busy.
- `./kit fn 0x0029b050` (input): only event 0x36, and only when not busy. It plays the shown
  part's out animation.
- `./kit fn 0x0029b1a0` (animation done): the in animation's end clears busy. The out animation's
  end closes that page. After the preload page, it opens the clutch page if one is due, else
  "RaceResultsMenu".
- `./kit fn 0x0029b3f0` (title): "Player 1 and Player 2 %s" when both seats missed, "Player 1 %s"
  when seat 0 did, else "Player 2 %s", with "Preload Warning:" or "Clutch Warning:".
- `./kit fn 0x0029c690` / `0x0029cc80` (results open): event byte +0x116 == 0 (single race) builds
  two buttons: STR_QUIT (icon 1, event 0x38) and STR_RESTART (icon 0, event 0x36). Otherwise it
  builds one: STR_NEXT (icon 0, event 0x36).
- `./kit fn 0x0029cde0` (results input): 0x38 in a single race opens the "Warning" / "Are you sure
  you want to quit?" dialog. 0x36 in a single race goes to the HOFMenu when a mark is waiting.
  Otherwise it commits the profile stats and restarts the race. 0x36 in a league goes to
  "RaceLeagueStandingsMenu". 0x3e/0x3f step the pages.
- Dialog yes (`0x0029e400`, `0x0029e550`): Hall of Fame first when a mark is waiting, else commit
  the stats and go to the front end.

## How it was proved

- Read in the PS2 ELF (above). The strings were read off the ELF with `./kit data`.
- `tests/menu.rs` walks it all by pad reports: the two P330/P333 tests, 300 menu tests passing.
- No function oracle: this is menu flow and text, and the menus are ours to design (AGENT.md). The
  sim-side words it reads (`RaceEnd::tips`, the Hall of Fame marks) were oracled in P330.

## Not verified / not 1:1

- **The popup's timing.** The original ignores OK while a page animates in, and changes page only
  after the out animation ends. Those lengths live in the UI scene's movies, which were not read.
  Ours takes Cross at once and changes page at once.
- **The popup's pictures.** The original plays the "Preload Movie" / "Clutch Movie" parts. Ours
  shows the title and line as text on our own page, without the movies.
- **Triangle as Quit is a guess.** The bar draws Quit with button icon 1 and its event is 0x38.
  Restart and OK use icon 0 and event 0x36, which is Cross. Which pad button icon 1 / event 0x38
  is was not traced; Triangle is the usual one. Ours also takes Circle, as every openmx back does.
- **Which dialog notice calls which listener** (`0x0029e400` vs `0x0029e550`) was not traced. Ours
  does what both together read as: yes → Hall of Fame first if a mark is waiting, then the main
  menu; no → back to the board.
- **The stats commit is not ported.** Restart and Quit commit profile stats 0–5 first. Our records
  count only races and wins. Filed as P358.
- **League mode does not exist here.** `Results::league` is always false. Next goes to the main
  menu because there is no standings screen. It is covered only by the test setting the flag.
- **Restart after a typed Hall of Fame name with no table taken** goes to the main menu, not the
  race. This cannot happen when the entry page shows (it shows only for earned records), but it was
  not proved.
- **No live check.** No PCSX2 capture of the results menu or popup was compared.

## Next

P358: the stats commit.
