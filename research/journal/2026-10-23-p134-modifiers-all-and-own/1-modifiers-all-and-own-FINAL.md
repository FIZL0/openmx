# 1 — modifiers all and own (FINAL)

Task P134. 2026-10-23.

## What I did

- **Two ways into Game Modifiers.** The host's page (1P offline, the host online) has **All Players**
  at the head of its Player row (`menu_pages.rs`, `mods_seat() == None`). A knob set there is every
  seat's row; a knob whose seats differ shows **Mixed**, and a step from Mixed sets every seat from
  1P's value. The world's knobs (gravity, air drag, time scale) are only the host's. The page still
  opens on 1P, not All, so the old 2P-only flows and tests work as before.
- **Every other player's own page.** Under the pause, a pad other than 1P's
  (`pauser != 0`) gets that seat's page. P84, the per-player pause, has not landed, so this is the
  shared pause the player's own Start opened. An online guest gets its machine's 1P page, in the
  ride and in the lobby. The Player row is fixed to the player's own seat and greyed out, and the
  world's knobs show their value but are greyed out with "only the host sets it". Online, the
  biplane knobs are greyed out too (see below). Presets and Reset to Original touch only the knobs
  the page can set.
- **Presets per seat or for all.** Save Preset on one seat's page keeps that seat's values for every
  row, so it loads onto any seat as it was. On All Players it keeps every seat's. Load on one seat
  sets that seat's row; Load on All sets every row. The existing test that saved on 2P and expected
  the whole set back now checks the per-seat preset.
- **Ways in that P118 dropped.** P118 removed the old front end's "every player's" choice on this page
  (All Players puts it back). Before this task the only way in was Settings → Ride → Game Modifiers,
  from the main menu or the pause. The pause now has its own **Game Modifiers** row (after
  Settings), and both lobbies (host and guest) have a **Game Modifiers** item before Leave. Ride
  setup still has no row. It has never had one, and the Ride tab sits next to it.
- **Online, a guest's own change** (`Netplay::remodify_own`, new `Msg::Mine` over the reliable link):
  the guest sends its own set (`own_set`: its seat knobs only) to the host. The host lays it onto the
  guest's seats (`with_own`) and takes it as its own change through P100's agreed restart, so every
  machine changes at one tick. The host's own pause change goes through `Online::remodify(to, all)`:
  the world and biplane knobs come from the host's set, the host's seat gets its own values, the
  knobs set through All Players go to every seat, and the guests' rows are left as they were
  (`host_set`). `rides::Modded::remodify` now rebuilds each seat on its own row, where before every
  seat took row 0.
- **Lobby.** Each pick carries the player's own set (`Pick.modifiers`, serde default ""). Every
  frame the host gathers the room's set from them, one row per seat (`Menu::lobby_mods`,
  `Lobby::gather`): a seat takes its pick's set only when that pick's text changes, so the host's
  All Players changes are not overwritten. The host sends the result with the new
  `Cmd::Modifiers`. Every machine builds each seat's bike on that seat's row (`main.rs`
  `mods.bike(i, …)`). Each seat's line in the lobby shows its first changed knob and how many more
  (`Lobby::seat_mods`).
- The game-side helpers live in `game_sim::modifiers::online` (pure functions on `Mods`). The menu is
  compiled into the test crates by `#[path]`, so it can read them without the `net` module.
  `net::rides` re-exports them.
- Settings are still saved per seat in settings.txt, in the same `modifiers =` line format as before.

## What the original does

Nothing to port: the original game has no modifiers. The old Rust front end (before P118) had an
"every player's" choice on its page, which All Players restores.

## How it was proved

Tests in crates/openmx:
- `tests/menu.rs` `the_hosts_all_players_sets_every_seats_knob`: on the host's page, All Players sets
  every seat's bike speed. After 2P alone changes, the page shows Mixed, and a step from Mixed sets
  every seat again. Only knobs set through All are handed to the shell, and Reset on All resets
  every seat.
- `tests/menu.rs` `a_guests_page_changes_only_its_own_seat_and_not_the_world`: on 2P's pause
  (2P's own pad), the page changes only 2P's row. It cannot move gravity or pick another player,
  and Reset touches only 2P. An online guest's page sets its own grip but not gravity, and the
  biplane knobs are greyed out.
- `tests/modifiers.rs` `two_local_seats_ride_on_their_own_bike_speeds`: two seats on one machine.
  With 2P at x2, 2P tops out at 171.6 ft/s against 1P's 88.3. Then 1P's own x3 is applied through
  `resim` seat by seat: 1P reaches 238.0 ft/s and 2P's top speed stays put.
- `tests/netplay.rs` `a_guests_own_change_both_peers_take_at_the_same_tick`: over a lossy, laggy
  link, the guest asks for bike speed 200 plus a gravity it is not allowed to set. Both machines
  take it at the same tick (`remods` equal). Only seat 2 changes; gravity does not. A later change
  of the host's own grip lands the same way. Every seat's bike is built on its own row, and the
  two machines agree on hashes and pads across both changes.
- `tests/lobby.rs` `the_lobby_gathers_every_seats_own_modifiers`: two real LAN menus. The guest's own
  speed reaches the room's row for seat 2 on both machines, and the world is the host's. The host's
  All Players grip reaches both seats, and the guest's lobby page greys out gravity and the Player
  row. Both lobbies show "Bike top speed 150%" on the guest's line.

## Not verified / not 1:1

- **Per-seat modifiers online only in rides started from the lobby.** A freeride opened to online
  (N10) and `--peer` rides still ride 1P's set on every seat, and a guest's own change is refused
  there with a note. Seats joining such a ride are decoded onto the first seat's bike, so a row per
  seat would make machines disagree until the next change. Changing that needs the join's spawn
  bytes to carry the seat's set.
- **Online, the biplane knobs are the host's for every seat.** Every seat flies a copy of the ride's
  one plane, built from seat 0. A guest's page shows those knobs greyed out. Offline, every seat
  still has its own.
- **Seats 8 and up share row 8.** `Mods` has 8 rows. A guest on a seat past the eighth in a ride with
  more seats cannot change its own, and the host refuses with a reason.
- **P84's per-player pause has not landed.** "Their own pause" here is the shared pause opened by that
  player's Start. When P84 lands, its pause should open the same page (`mods_own` reads `pauser`).
- **Not seen on screen.** The side panel's tag reads `All` or `2P` (`menu_screens.rs`). The lobby
  item and seat lines were checked only as strings in tests; no shot was taken.
- **Lobby gather timing.** The host merges a guest's pick only when that pick's text changes. If the
  host sets a knob through All Players and the guest then changes any knob of its own, the guest's
  whole own set replaces its row, including the knob the host set for everyone. This is
  deliberate: the guest's last word on its own seat wins.
- **An online guest's change from the lobby page reaches the room once the guest's pick is resent.**
  The pick goes out with the controller-preset line every menu frame, as markers and machine
  already do. This is not tested beyond the lobby test above.

## Next

Nothing for this task. Follow-ups are the two online limits above (drop-in rides per seat, biplane
per seat).
