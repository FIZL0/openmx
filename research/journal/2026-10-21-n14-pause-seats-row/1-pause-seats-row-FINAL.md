# 1 — pause seats row (FINAL)

Task N14. 2026-10-21.

## What I did

- **The pause's Seats row** (`menu.rs`, `PAUSE_SEATS`): a ride that can be opened to online
  (`Opening::Can`) or is (`Opening::Open`) has "Seats: n" under the online row, so Quit moves down
  one more. Left, right and Cross step it round from the cap to two, as the lobby's Seats row does.
  - **Before opening**, it only sets `Settings::lobby_seats`, which `main.rs` already opens the room
    with. It is saved like the lobby's pick (`changed`), and the network is not told anything.
  - **Once opened**, the row shows the room's own count once the network has sent an `Event::Room`
    for a riding drop-in room. Until then, or if the room left over in `lobby.room` is an old lobby,
    the row shows the pick. A step pushes `Cmd::Seats(n)` and records `n` as the pick.
  - The pause's ticker shows the network's status while the ride is opened. That is how a refused
    resize, such as "Seat 3 is taken: 3 seats at least.", gets to the host.
- **Getting the command to the network during the ride** (`main.rs`, `online.rs`): while a ride is
  opened, nothing pumps the front end's `Network`. So the frame that hands `on.kept()` to the menu
  now also takes `menu.lobby.out` and passes it to the ride's lobby connection through the new
  `Online::command`.
- **The host task** (`openmx-net/src/net.rs`): `Cmd::Seats` used to be refused whenever
  `room.riding`. It is now taken when `!room.riding || room.drop_in`. A riding room that takes nobody
  on (a lobby's started ride) still ignores it. `Room::resize` already refuses to shrink a room below
  its last taken seat, so a peer who has dropped in keeps their seat. The listing is marked dirty, so
  Find shows the new count.
- **Tests**:
  - `tests/lobby.rs`:
    - `an_opened_rides_pause_has_a_seats_row`: the pick before opening, wrapping from 2 to the
      cap, `Cmd::Seats` once opened, and the row following the room's `Event::Room`.
    - `a_freerides_pause_opens_it_online`: changed for the new row.
  - `openmx-net/tests/netplay.rs`: `an_opened_ride_resizes_its_room_and_a_closed_one_does_not`. A
    real LAN host task resizes a riding drop-in room from 4 to 9 and refuses 1 with its reason. A
    riding room without drop-in stays at 4.
- **From opentennis**: nothing. Its lobby is fixed at four and has no mid-match join (N10 and N12
  checked).

## What the original does

Nothing: the original has no online play. The pause row is this port's own front end.

## How it was proved

- `./kit check` passes. I also ran each new test on its own (`--test netplay an_opened_ride`: 1
  passed; `--test lobby pause`: 2 passed), so I know they ran and were not skipped.

## Not verified / not 1:1

- **The shell path was not run live.** The row and its command are tested in the menu, and the host
  task is tested over a real LAN endpoint. The `main.rs` forwarding of `lobby.out` to
  `Online::command` during an opened ride was only compiled. As with N10, no shell was opened with
  the pause driven over a live room. The unattended `--play` tape rides, and it does not walk the
  pause onto an opened ride.
- **Growing the room mid-ride was not ridden with joiners.** More seats only let more peers in
  through N10's admit path, which N10 and N12 tested. I did not run a session where a peer joins
  after a mid-ride resize.
- **A refused step still moves the pick.** If the network refuses a smaller count (a taken seat),
  `Settings::lobby_seats` keeps the refused number, while the row shows the room's real count. This
  is the same as the lobby's own Seats row (N12), and I kept it that way so the two rows behave alike.
