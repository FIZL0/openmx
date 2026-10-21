# 1 — local two players (FINAL)

Task P51. 2026-10-21.

## What I did

- Went through each suspect the brief names. None of them needed four players:
  - `Join::press` (join.rs) returned `Done` on Start from any seated controller, whatever the count.
  - `walk_join` (menu.rs) builds the `Party` from `Join::views()`, which is the seats in play, not a
    layout's count, and `riders` comes from the same list.
  - `layouts(2)` is the original's two (Stacked, SideBySide), `layouts(3)` is Quarters, and
    `Layout::for_players` maps 1/2/3-4/5-8 to Full/two-way/Quarters/Eighths.
  - In the ride (main.rs), `args.players = party.players()` and `pad_of = party.devices`. The views are
    `rects.iter().take(args.players)`. The software path blanks a spare view black (`split::blank`), and
    the GPU path clears the frame black and draws only `world.seats.len()` views, so three players
    leave a dark fourth quarter.
  - The code at HEAD therefore already rode 2 and 3 players. I could not reproduce the user's "won't
    start with fewer than four" from the code. The one rule missing from the brief was the opposite
    case: a host **alone** could press Start and get a "local game" of one.
- join.rs: added `FEWEST = 2` and `Join::can_start()`. From the menu, Start is `Done` only once two
  seats are in play, and a gone seat still counts because its rider waits on the line. From the pause
  it is always `Done`, because the ride goes on with whoever is left. Otherwise Start gives `Stay`.
- menu.rs: the join screen's legend shows "Ride" only when `can_start()`. `Menu::open(Screen::Split)`
  (for `--screen split`) seats pad seat 1 beside the host so the screen still opens.
- Tests:
  - `tests/menu.rs`: `two_or_three_pads_start_a_local_game_with_a_view_each` covers two pads under both
    `split` settings (exactly two views, the setting's layout, devices [0, 1]). It also covers three of
    four pads (Quarters, 4 rects, devices [0, 3, 1] in join order) and four.
  - `tests/menu.rs`: `one_pad_cannot_start_a_local_game_but_can_free_ride` covers one pad alone, and
    one of two plugged in; also join and leave, then Start stays on Join; then back to the main menu
    and a Free Ride with `party()` none.
  - `tests/join.rs`: `start_needs_the_host_and_one_more`.
  - The pad-only reachability BFS now starts with a second pad seated (via `Join::with`), because it
    taps one pad and the split screen now needs two.
- opentennis: its Local assignment (`assign.rs`) was already P43's model. Nothing new was taken for
  this one.

## What the original does

Not consulted: the front end is ours to design (AGENT.md). The two-player layouts are the original's
(V22's `h`/`v` presets) and unchanged.

## How it was proved

By pad input alone through `menu::Reader` on virtual pads (report bytes per pad seat, as main.rs's
front-end loop feeds `present`/`step_seat`/`step`), in the `tests/menu.rs` and `tests/join.rs` tests
above. `./kit check`: TESTS PASSED. Filtered runs confirmed the new tests ran (6 and 5 passed).

## Not verified / not 1:1

- I never reproduced the user's report live. The real shell skips uinput pads (memory
  shell-skips-uinput-pads), so no live join could be driven with more than one pad. Reading the code
  found nothing at HEAD that required four. If the user's case came from something outside the menu,
  such as how their pads land in `PadSlots` seats, these tests do not cover it.
- The menu tests check the ride's view count through `Party::layout().rects()` and `riders`. They do
  not start `ride()`. That the ride draws exactly that many views and a dark spare quarter comes from
  reading main.rs (`views_of`, `split::blank`, the GPU's black clear), not from a test.
- Up to 8 (V23) is untouched: the screen still offers `ON_SCREEN` = 4 seats.

## Next

Done unless the user's four-player hold shows up again live. If it does, log `local game: …` from
main.rs and the join screen's `present` list.
