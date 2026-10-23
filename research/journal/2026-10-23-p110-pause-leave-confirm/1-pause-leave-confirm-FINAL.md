# 1 — pause leave confirm (FINAL)

Task P110. 2026-10-23.

## What I did

- `menu.rs`: the pause's Main Menu row and its Quit row no longer act on one press. They open a confirm
  (`Menu::leaving`, a `Leaving::{Main, Quit}` and a row) that starts on Stay (row 1), the way the splash's
  Quit popup does (`QUIT_ROWS`). While it is up, `Menu::step` hands the pause's press to `Menu::confirm`
  before the row walk: any direction swaps rows, Cross on the leaving row returns `Act::MainMenu`
  (`self.main()` as before) or `Act::Quit`, and Cross on Stay, Circle or Start close it, back to the pause
  on the row it was. `pause_for` and `main` clear it, so a new pause never opens on an old confirm.
- Wording: `Menu::leave_to: LeaveTo` is set by the shell. `Main` → "Main menu?" / "Main Menu" (a local
  ride, or a freeride opened to online, whose room closes with it); `Lobby` → "Return to lobby?" /
  "Return to Lobby" (the host of a lobby's ride, whom `Online::leave` sends back to the lobby); `Online` →
  "Leave the online game?" / "Leave Game" (a guest, whom `Online::leave` takes out of the lobby — they
  land on the Online screen, not a lobby, so "Return to lobby?" would have been untrue for them). Quit is
  "Quit the game?" / "Quit", as on the splash. `Menu::confirming()` gives the drawing all three.
- `online.rs`: `Online::leaves_to()`, the same three-way split `Online::leave` makes. `main.rs` sets
  `front.menu.leave_to` from it where it sets `opening` / `mods_lock` (offline: `Main`).
- `menu_screens.rs`: the splash's Quit popup body became `Painter::popup(ask, rows, row)`, and the pause
  draws it over itself while confirming, its hint reading Circle "Stay".
- Over a split nothing new was needed: `step_seats` already hands an open pause only the pauser's press
  (and the keyboard rides with 1P, as it already did for the pause).
- opentennis has `crates/hst/src/play/main_menu/confirm.rs`; I did not take it — openmx's own splash
  popup already had the look and the two-row logic, so the confirm reuses that instead.

## What the original does

Front end: ours to design (AGENT.md). Not compared against the original's pause.

## How it was proved

Unit tests in `crates/openmx/tests/menu.rs`:
`leaving_from_the_pause_asks_first_on_stay` (each of the three wordings: Cross shows the confirm on Stay
with the pause still up and only `Act::Stay`; Cross on Stay, Circle, Start each back to the pause on the
same row; Up/Down swap; only the leaving row gives `Act::MainMenu`; the pause's Quit likewise gives
`Act::Quit` only on confirming; a fresh pause has no confirm) and
`over_a_split_only_the_pausers_pad_answers_the_confirm` (2P's pause: 1P's Up, Circle, Cross do nothing,
2P's Cross leaves). The old walks that pressed Main Menu / Quit on the pause (`menu.rs`'s main walk,
`lobby.rs`'s `a_freerides_pause_opens_it_online`) now confirm. `./kit check` passed.

## Not verified / not 1:1

- The confirm's picture was not looked at: `--screen pause` cannot open it, and I did not add a shot
  path for it. It is the splash's Quit popup's drawing, unchanged, so it should look the same as that one.
- The shell's `leave_to` wiring (`Online::leaves_to` → host Lobby / guest Online / opened freeride Main)
  is not exercised by a test with a live online ride; it mirrors `Online::leave` branch for branch.
- Online, the ride goes on under the confirm for the others (it is not frozen online, as the pause never
  was); offline it stays frozen under the pause, as before. Not new behaviour, just stated.

## Next

Nothing.
