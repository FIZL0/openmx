# 1 — multiplayer main menu (FINAL)

Task P43. 2026-10-20.

## What I did

- **The main menu** (crates/openmx/src/menu.rs `MAIN`) is now Free Ride, Local Game, Online, Garage,
  Settings, Quit. The order follows opentennis's `main_menu/home.rs` (Local, then Online beside it).
- **Local Game** opens a new `Screen::Join` over P38's `join::Join<usize>`. The device id is the shell's
  pad seat number (P35's `Seats` slot), because P35 keeps that number for a pad across an unplug.
  - The pad that chose Local Game is seated as 1P at once, as opentennis's host is.
  - Every other controller presses its own way in through `Menu::step_seat`, using P38's rules
    unchanged: Cross joins, Left/Right move to an open seat, Circle leaves, and Square lets go of seats
    whose pad is gone.
  - Start from a seated pad is done, and Circle from an unseated one goes back.
  - The shell feeds `Menu::present` every frame with the pad seats that are `Playing`. A pulled pad's
    seat shows as gone and still counts as a view.
- **Start goes to a new `Screen::Split`**, the split layout. Its rows are `menu::layouts(n)`: the
  original's two for a pair (one above the other, side by side), and the port's single layout for any
  other count (full, quarters, eighths). A picture of the views, numbered 1P..nP, sits right of the
  list. The pair's pick is kept as `split = h|v` in `settings.txt` (`Settings::split`).
- **Then the existing ride setup** (Tracks) lists a machine per player (`riders`) and starts the ride.
  `Menu::party()` is the local game: `devices` (player i's pad seat) and `split`. Free Ride clears it
  and puts `riders` back to `free_riders` (`--players`).
- **main.rs: the shell reads the party.** When `party()` is set, the front end sets `a.players`,
  `a.split` and a new `Args::pad_of`.
  - In the ride, player i reads pad seat `Args::pad_seat(i)`. That applies to its report, its scheme
    (P40), its "player N: pad" line and the waiting-freeze. Outside a local game `pad_of` is empty and
    `pad_seat(i) == i`, so every other ride is exactly as before.
  - The front-end loop fills `Menu::devices` with each pad seat's name (or "Keyboard") for the cards,
    and prints `local game: N players on pad seats [..], <layout>` when one starts.
- **Online** has no screen until N3. Its row puts a note on the ticker (`menu::ONLINE_NOTE`) saying the
  menus don't have it yet and to use `--online`. N3 replaces that arm with its lobby.
- `join.rs`: only `PartialEq, Eq` added to `Join`'s derive, because `Menu` derives them.
- Tests (crates/openmx/tests/menu.rs):
  - `two_and_four_players_join_pick_a_track_and_ride_that_many_views` drives the menu from the main
    menu by reports alone, through a copy of the shell loop's per-pad-seat stepping (`front_frame`).
    - Two players: pads 0 and 2 of four plugged in. Pad 2 joins and moves right, and pads 1 and 3
      press things and stay out. They pick side by side and Norfolk, and the ride starts with party
      devices `[0, 2]`, layout SideBySide and two 320x448 views. Then Free Ride is one player again.
    - Four players: all join, the fourth is pulled before Start and keeps its view, and the ride gets
      four 320x224 quarters.
  - `backing_out_of_a_local_game_walks_its_screens_back_to_the_main_menu` covers the way back and the
    Online row.
  - Existing tests changed only where the main menu's row indices moved (Settings and Garage are
    further down). The pill test counts the split layout's list too (6 list screens, not 5), and the
    settings round trip carries `split`.
- From opentennis: the screen order and the auto-seated host, from `home.rs`/`assign.rs` and B40h's
  journal. Changed: no swapping a seat with another player (P38's rule), no Mods/Extras rows, and the
  split layout is a screen of its own, because MX Unleashed's original lets the player choose a pair's
  split and tennis has none.

## What the original does

Not applicable. AGENT.md leaves the front end to us. R9/V22 already say what the original's two
layouts are (`split.rs`'s header), and the split screen offers exactly those two for a pair.

## How it was proved

- `./kit check`: 829 tests passed. That includes the two new menu tests above and the disc-drawing
  tests (`every_screen_draws_from_the_disc`, `every_pill_fits_its_label_and_nothing_covers_the_logo`),
  which ran because this worktree's iso link is present. Both of those now cover Join and Split
  through `Screen::ALL`.
- By eye: `openmx --menu --screen main|join|split --size 640x448 --shot …` on :0. The main menu's six
  rows clear the logo, the join screen shows four cards (Keyboard as 1P, "✕ to join" on the rest), and
  the split screen shows its list and the view picture.

## Not verified / not 1:1

- **The shell end to end was not run with two or four real controllers.** This machine has no
  readable pad for this slot ("pad: none found"; js0 is another slot's idle pad), and no keystrokes
  reach the window. So nobody pressed through Local Game into a live split-screen ride. What is tested
  is the menu state machine (the same calls main.rs makes, in the same order) and the party it hands
  over. `Args::pad_seat`'s use in the ride loop (report, scheme, freeze) was read, not run with a
  remapped seat. V22's `a_four_player_session_steps_and_draws_four_views` still proves that
  `--players 4` draws four views.
- **The keyboard can join only when it holds a pad seat.** With pads plugged in and no `--pads`, P35
  puts the keyboard beside 1P's pad on seat 0, not on a seat of its own, so it is not a separate
  controller on the join screen. Who drives the menus is P46's task.
- **Escape on the join screen is pad seat 0's Circle.** If seat 0 is seated it leaves that seat first,
  and only a second Escape goes back. The keyboard is not an independent device there (the point
  above).
- **Online is a note, not a screen,** until N3 lands its lobby.
- **Four on one screen, not eight:** `join::ON_SCREEN` (4) until V23. The pause menu's join (P38's
  "from the pause menu") is not wired; that remains P38's.
- **Not drawn: the split picture for two and four.** Only the one-player picture (`--screen split`
  opens with one) was looked at. The two- and four-player rows were checked by the pill test's list
  geometry, not by eye.

## Next

Nothing for P43. P38 is still open for the join screen from the pause menu (a joined or left seat
spawned or removed on Resume) and the pad-drop display mid-ride. It can reuse `Screen::Join`,
`Menu::present` and `Menu::step_seat` from here.
