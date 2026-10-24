# 1 — freeride drop out online (FINAL)

Task N11. 2026-10-23.

## What I did

Until now, an online ride ended for everyone as soon as one player went. Now the others ride on.

- **Leave Session in the pause.** The pause now has a Leave Session row (`menu.rs`, `PAUSE_LEAVE_ROW`) whenever the ride is online, after Open to Online / Seats and before Quit. It asks for confirmation first:
  - The machine's first player: "Everyone on this machine goes; the others ride on."
  - A second local player on the same machine: only that player's seat goes.
  - `Act::LeaveSession(pauser)` goes to `Online::leave_session`, and from there to `Netplay::leave`.
- **The session side** (new `net/leave.rs`):
  - A guest sends `Msg::Leave { seats }` to the host. The host makes the next restart without those seats: its settled state plus the new line-up, the same restart `recover.rs` makes for a silent peer and `admit.rs` makes for a joiner. Every machine, the leaver included, takes that restart at the same tick.
  - The host no longer forgets a leaver before the restart reaches it, so the leaver gets it too.
  - A leaver with no seats left finishes with "you left the ride". `Online::frame` keeps passing datagrams until the link owes nothing (`flushed`, at most 5 s), then lets the lobby go.
  - A leaver that hears nothing back for 5 s goes anyway. The others then drop it as a silent peer.
- **The host leaving.** It empties its own seats the same way. The restart's line-up then has a new host: `Lineup::host`, the first seat that is still taken. `leave.heir` names that peer, and the shell sends `Cmd::HandOver(heir)`.
- **A silent host** (`recover.rs`): this used to end the ride. Now the next peer in seat order takes over after `silent + wait + rank × host_wait`. It empties the host's seats from its own settled state and sends the restart as the new host. The others accept a restart from a peer other than the host only when the old host is not in that restart's line-up.
  - A game that cannot encode its state still ends the ride, but the reason now names the host.
- **A silent guest** was already dropped by `recover.rs` and the ride went on. The transport no longer overrides that by sending everyone back to the lobby: `Guest::Gone` in a riding room now frees that guest's seats.
- **The transport** (`openmx-net` `net.rs`, `room.rs`):
  - The room keeps each seated guest's dial ticket (`Room.tickets`, sent in `Hello`), and `Room::successors` lists the seated peers in seat order.
  - A host's `Cmd::HandOver(to)` broadcasts `Msg::Handover` and closes.
  - A guest treats any of these as "the host is lost": the handover message, a stream error while riding, or 6 s of silence while riding (`HOST_GONE`).
  - It then walks the successors:
    - The first one, if it is this machine, hosts the same room under the same code: `host(.., me)`. It republishes the code (its pkarr key comes from the code) and lets the others back into their seats without a new handshake for 20 s.
    - Otherwise it dials that peer by ticket, or by endpoint ID if there is no ticket.
  - Both cases end with `Event::HandedOver(new_host)`. The menu shows "The host left: you host the game now." / "… {name} hosts the game now.", and `hosting()` follows it.
  - If no successor answers: "The host left, and no other rider could take the room over."
- `crates/openmx/tests/lobby.rs`: two existing tests expected a guest's leave to send the host back to its lobby. They now check that the host rides on with one seat. One of them, the in-step one, now leaves through Leave Session; its name changed from `…a_leaver_ends_it` to `…a_leaver_lets_the_host_ride_on`.
- `crates/openmx-net/tests/netplay.rs`: the LAN test likewise now expects "Guesty left the ride." with the room still riding.

## What the original does

Nothing to compare: the original has no online play. The session model follows opentennis's `recover.rs`, the same prior art N2/N10 used.

## How it was proved

- `crates/openmx/tests/netplay.rs`. Headless sessions over the in-test lossy link, comparing per-tick checksums:
  - `of_three_peers_one_leaves_from_the_menu_and_one_is_killed_and_the_ride_goes_on`: host plus three peers. One peer leaves through `Netplay::leave`, then another is dropped without a word. The two left ride on with matching checksums for over 450 ticks. The leave's tick is within −10..+30 of the ask; the killed peer's seats go about `wait` after its last tick.
  - `the_host_leaving_hands_the_ride_to_the_next_seat`: the next seat becomes host. The old host is `flushed`. The others agree for over 400 ticks, and a peer killed afterwards is dropped by the *new* host.
  - `a_silent_hosts_ride_is_taken_over_by_the_next_seat` and `a_silent_host_whose_ride_cannot_be_handed_over_ends_it` (a `Mute` game, which cannot encode its state).
  - `a_local_seat_on_an_online_peer_leaves_and_its_machine_rides_on`.
- `crates/openmx-net/tests/netplay.rs`, over real iroh endpoints on localhost:
  - `the_host_hands_its_room_to_the_next_seat`: after the handover, the room code leads a new joiner to the new host.
  - `a_host_that_goes_without_a_word_leaves_its_room_to_the_next_seat`: the host's `Net` is dropped.
- `crates/openmx/tests/lobby.rs`:
  - `an_online_rides_pause_leaves_the_session`: the row, both confirm texts, the act, and no row for a local ride.
  - The in-step lobby ride: two real LAN `Net`s and two `Online`s. The guest's Leave Session ends its ride with "you left the ride", and the host keeps framing without error, down to one rider.
- `./kit check --all` (see the commit).

## Not verified / not 1:1

- **The transport's 6 s silence detection (`HOST_GONE`)** is not exercised by a test. Every test host goes by closing its endpoint, which the guests see as a stream error. There is no way in-process to cut the network while keeping the connection open.
- **The session and the transport each pick the next host on their own.** The session uses the first taken seat in the line-up; the transport uses `Room::successors`, which lists the room's seats in order. They agree when the line-up is built from the room's seats, as for lobby and opened rides. Nothing checks this, so a room whose seats ever differ from the session's line-up would hand the ride and the room to different peers.
- **A handed-over lobby ride (not opened to drop-ins) cannot take new joiners.** The new host has no spawn state for them, which is how it was for the old host too. A handed-over *opened* ride admits joiners with the old host's first seat's pick (`Online.spare`); that path has no test.
- **The pause's Main Menu still does the old leave.** It sends `Cmd::Leave`, or `Cmd::Lobby` for the host. Now that a dropped guest is ridden on without, a guest leaving this way costs the others a `wait`-long stall rather than ending their ride. The host's Main Menu still takes everyone back to the lobby, on purpose: that is "end the ride".
- **The shell's frame loop end to end is not run.** That is a real Leave Session picked in a rendered pause, through `main.rs`'s `Act::LeaveSession` arm and back to the menu. The parts are each tested: the menu's act, `Online::leave_session` / `frame` over a real LAN `Net`, and the session. The `main.rs` glue only shows that it compiles.
- **`peer.rs`** (the headless browser/desktop peer) ignores `Event::HandedOver` (its `_ => {}` arm). It has no Leave Session and no handover test.
- **Rejoining:** a peer that left cannot rejoin the same ride unless the ride is open to drop-ins (N10).

## Next

Nothing for this task.
