# 2 — the lobby screens, the disc's hashes, the ride over the lobby (FINAL)

Task N3. 2026-10-20. Picks up entry 1's Next 1 and 2, now that P36 (the front end) and N2 (rollback) are in main.

## What I did

- **Online screens in P36's menu** (`crates/openmx/src/menu.rs`). They are drawn by the same `Painter` and in the
  same style: logo, column list, legend, ticker.
  - The main menu is now Free Ride / **Online** / Settings / Quit.
  - **Online** has Quickplay, Host, Find Games, Join by Code, and a **Network** row (LAN only / Internet and LAN).
    The Network row writes `online.txt`'s relay and restarts the network the next time it is used.
  - **Find Games** lists `Event::Found`; Cross joins one, and the last row refreshes.
  - **Join by Code** types the code on P36's on-screen keyboard. The keyboard now takes its title, length and
    capitalisation from what it is typing for (name 12, code 8, chat 40).
  - **Lobby**:
    - A side panel shows the room code to pass on, every seat's name with its rider and bike set, ready and
      ping, and the last five chat lines.
    - The host's rows are Track, Wrecks, Bike, Private, Chat, Start and Leave. A guest's rows are Bike, Chat,
      Ready and Leave. Before the room exists there is only Leave.
    - Circle leaves.
  - The menu is still a pure state machine. Online screens queue `openmx_net::Cmd`s in `menu.lobby.out` and
    take `Event`s through `Menu::heard`, so the tests drive it with no network at all.
  - The flow follows opentennis's `main_menu/online.rs`: host / join by code / quickplay / LAN, the lobby list, and
    back to the lobby with the reason as a note.
- **`crates/openmx/src/lobby.rs`**:
  - `DiscFiles` is the `openmx_net::Disc` the shell hands the network.
    - A bike set's hash is FNV-1a over its `.phy`: the first `Bike\<model>.res` in the disc's order that carries
      the set's file.
    - A track's hash is over `<track>.res` + `<track>_2.res`, the shortest member names.
  - `Network` starts `Net` lazily, sends the player's name before Host/Join/Quick, forwards commands, and feeds
    events to the menu once a frame.
- **The ride from the lobby** (`online.rs` `Online::lobby`, `main.rs`):
  - N2's rollback session runs over the lobby's connection. A datagram of 1000 bytes or less goes as
    `Cmd::Datagram`. A bigger one goes on the reliable stream as `Cmd::Send`, addressed to its peer. N2's
    datagrams go up to 1200 bytes; openmx-net's limit is 1100.
  - Every seat rides its own pick's `.phy`, the same bytes that were hashed, so what the handshake compared is
    what is ridden. The seed is the room's.
  - **Back to the lobby when a peer drops**: `Dropped`, `Failed`, or the room no longer riding ends the ride.
    What ended it is kept, given to the menu, and the shell returns to the Lobby screen, or to Online if the
    lobby is gone.
  - From the pause, Main Menu sends the host's `Cmd::Lobby` (everyone back to the lobby) or a guest's `Leave`.
- `Pick` derives `Default` (openmx-net).

## How it was proved

`crates/openmx/tests/lobby.rs` (`./kit check -- -p openmx --test lobby`, passing; whole workspace `./kit check`
TESTS PASSED):
- `the_online_screens_go_by_pad_and_the_network_s_word_alone`: pad reports alone.
  - Host gives `Cmd::Host` with this player in the host's seat.
  - Track right gives `Cmd::Ride`; bike right gives `Cmd::Picks`; Start gives `Cmd::Start`.
  - The code typed on the keyboard gives `Cmd::Join("AB2C", picks)`.
  - Ready, and chat out and in, both work.
  - `Event::Start` gives `Act::Online`.
  - `Dropped` then the room not riding brings it back to the Lobby with the note. `Failed` lands on Online.
- `the_network_row_switches_to_the_lan_only`.
- `a_host_s_menu_and_a_joiner_s_find_each_other_by_room_code_on_the_lan_and_ride`: two real `Net`s on LAN only
  (no relay, no board, no code server, so the code can only resolve over mDNS).
  - The guest's menu types the host's code on the keyboard and joins.
  - Both lobbies show both seats with their riders and bikes.
  - The guest says ready and the host's Start gives both menus `Act::Online` with one seed.
  - The guest leaves and the host is back in its lobby.
- `a_joiner_s_menu_whose_bike_set_differs_is_refused_and_told_why`: the guest's disc hashes race125p
  differently. Nobody rides, the guest lands on Online with a note naming race125p, and the host's room has
  only the host.
- `the_lobby_s_ride_goes_over_the_lobby_s_connection_in_step_and_a_leaver_ends_it`: after the start, two
  `Online::lobby` rides run 240 frames over the lobby's connection.
  - The host is on the gas and the guest leans.
  - Probed once: **230 ticks settled on both with equal per-tick hashes, 0 desyncs**.
  - The guest's leave then ends the host's ride ("Guesty left the ride: back to the lobby."), and its menu is
    back in the Lobby.
- `the_disc_s_hashes_are_of_what_the_ride_loads`, against the retail disc. A one-off panic probe confirmed it
  runs here and does not skip.
  - race125p's bytes read as a `.phy` and are what's hashed (`0x825d1f4edb43c384`).
  - race125p ≠ pro125a, all sixteen sets are found, and an unknown set is None.
  - The five freeride tracks hash differently from each other (free06 `0x0a9f5bacf0760dd1`), and an unknown
    track is None.
- The relay case and the mismatched bike over a relay are entry 1's tests in `crates/openmx-net/tests/netplay.rs`,
  still passing. This entry's tests are on the LAN only.
- `crates/openmx/tests/menu.rs`'s settings walk taps one more Down (Online sits above Settings now).

## Not verified / not 1:1

- **Not run as two shells on screen.** Every screen is proved by pad reports and the draw test (every screen draws
  over 90 % of the frame from the disc's art), but no two `openmx` windows were joined by hand. The `main.rs`
  wiring has no test: `Act::Online` → ride, the per-seat `.phy` swap, the pause's leave, and `kept()` back to the
  menu. It compiles, and the pieces it calls are tested.
- **The real internet is untested**, as in entry 1. Quickplay and Find over the DHT have no test.
- **One local player online**: the lobby seats one pick per machine, and the shell refuses a lobby ride with
  more than one local seat.
- **The drawn bike model is the shell's `--bike`** whatever set a seat rides. Only the physics (`.phy`) follows
  the pick.
- **Spawn and other per-bike setup** come from cloning seat 0's sim and swapping `phy`. Anything else `Sim::new`
  derives from the `.phy` stays seat 0's. It is the same on every machine (no desync), but it may not be what a
  ride on that bike alone would be.
- **The rider pick is cosmetic and fixed** to `--rider`; the lobby has no rider row. The biplane switch is always
  off in the lobby (`Cmd::Ride { biplane: false }`).
- **Restart is disabled online**. A guest that leaves from the pause leaves the lobby too. The host's Main Menu
  takes everyone back to the lobby.
- **Lobby size is 4 seats** (`LOBBY_SEATS`), my choice. openmx-net allows 8.
- **Track hashing reads both archives whole** on the network thread when a ride starts. It is fast on this
  machine's disc image; a slow medium against `SHAKE_WAIT` (10 s) was not tried.
- **`LOBBY_DATAGRAM` (1000)** is under openmx-net's 1100 by a guessed margin for the addressing. Neither limit
  was measured against iroh's path MTU.
- **Switching the Network row drops the old `Net`**. That assumes its thread ends when the command channel
  closes, which was not checked. Switching mid-lobby is not offered (the row is on the Online screen).
- **Chat is cheap but plain**: five lines in the lobby panel, nothing during the ride.

## Next

- Two shells joined by hand on screen (or an AT-SPI/tape-driven pair), and a test that drives `main.rs`'s lobby
  ride once the shell's run loop is steppable (N4 splits it).
- A rider row, and the drawn model following the picked bike set.
