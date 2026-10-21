# 1 — freeride open online (FINAL)

Task N10. 2026-10-21.

## What I did

- **The session takes a peer on at a tick** (`crates/openmx/src/net/admit.rs`). The host's
  `Netplay::admit(peer, seats, spawn)` queues a joiner. Nothing happens until the joiner's first
  datagram arrives (`Admit::heard`), so the host never waits on a machine that is still loading. Once
  it has, and no restart, resync or end is in flight, the host takes its settled state
  (`rollback::confirmed`), grows the game by the new seats (`Game::seat`, spawned from the state the
  ride started in), and sends the state and the new lineup as the usual `Msg::Restart` pieces (round + 1).
  Every machine, the host included, then goes through the same path as B196's recovery
  (`recover::restarted`): decode, a new ggrs session from that tick with the new player count, and the
  log and hashes truncated there. Nobody's ride goes back to tick 0. The joiner starts as
  `Netplay::joining` (round 255, lineup just the host), runs no ticks, and only keeps the host's link
  alive until the restart reaches it. `Rides::decode` and `Rides::seat` grow the seat list.
  `rollback::confirmed` also works with no remote player (a ride the host had alone).
- **The lobby side** (`openmx-net`): `Room::drop_in`. A riding room with it set is listed as joinable
  (`Listing::riding`, which is not serialised when false, so mDNS records keep their old size and
  older builds still read it). A guest's `In` to such a room is seated straight away with the first
  seat's bike set (`drop_in_picks`, since the ride has one `.phy`). It is sent `Start`, handshaken
  against the room, and then sent `Msg::DropIn(room)`. The host gets `Event::Arrived { peer, seats }`.
  A handshake failure or a timeout (`SHAKE_WAIT`) frees the seats. A peer that leaves a drop-in ride
  frees its seats without sending everyone back to the lobby.
- **The shell**: the pause menu of a local one-seat freeride on the disc's own `.phy` (no tape, no
  head, no `--phy`, not already online) gets an "Open to Online" row above Quit
  (`menu::Opening::Can`). Pressing it gives `Act::OpenOnline`. `main.rs` builds the room (track,
  wrecks, mods, the host's pick, `riding` + `drop_in`) and hosts it with `Network::open`. It then wraps
  the running ride in `Online::open`, which starts a session from the current tick with the host
  alone. The row then shows "Online: opening..." and then the code. A guest whose join lands in a
  riding room gets `Event::DropIn`, and the menu goes `Act::Online` with `lobby.drop_in` set.
  `main.rs` loads the room's bike and mods and starts `Online::drop_in`. Find shows
  "(riding: drop in)".
- **`peer.rs`** (the test and browser peer): `Peer::open`. A datagram over `DATAGRAM_MAX` (a restart
  piece plus a lineup of 64-hex iroh names) goes over the reliable stream, addressed, as the shell's
  `online.rs` already sends them. `Peer::too_big` counts these.
- Tests:
  - `tests/netplay.rs`:
    - `a_freeride_opened_online_takes_two_peers_mid_ride`: session level, mash pads. The host rides
      alone and is opened; `g` joins, then `k` joins with 2 seats. Checks: the host's hashes before
      the first join equal a never-opened run's; the host's tick never goes back except the one
      step to `k`'s join tick (`g` is taken at the settled tick); all machines agree from each join
      tick; restarts are counted; the lineup reads "host,g,k,k".
    - `a_ride_that_cannot_seat_a_joiner_says_so`
    - `a_freeride_opened_on_the_lan_takes_two_peers_mid_ride`: end to end, three `Peer`s over LAN
      mDNS, joined by room code mid-ride.
  - `tests/lobby.rs`: `a_freerides_pause_opens_it_online`, which covers the pause row, the code,
    and DropIn/Arrived/Start in the menu.
  - `room.rs`: `a_ride_open_to_drop_ins_lists_as_joinable`.

## What the original does

Nothing: the original has no online play. This is the port's own netplay (N2/N3/N4). The ride state
it hands over is the same `game_sim::snapshot` the rollback already saves.

## How it was proved

- Determinism: in the session test, the host's per-tick state hashes and pad logs are compared with
  each joiner's for every settled tick from the joiner's join tick to the end (~600 ticks). The
  host's hashes before the first join are compared with a never-opened run of the same pads.
- End to end: the LAN test compares the same per-tick `peer::ticks` lines across three real iroh
  endpoints (it passed in 24 s alone and in the full gate).
- `./kit check`: the whole gate passes.

## Not verified / not 1:1

- **The shell path is not run live.** The pause row, `Act::OpenOnline` and `Event::DropIn` are tested
  in the menu, and the session and transport are tested end to end with `Peer`. The wiring in
  `main.rs` (building the room, `Online::open` and `Online::drop_in` on the real track) was only
  compiled; no shell was opened with a second machine joining. Reason: two shells and a LAN join
  need two windows' worth of input. The unattended `--play` tape drives one shell, not the menu of
  a second one.
- **The host holds during each join.** After every restart, ggrs syncs again (5 round trips) before
  it advances. The host's ride holds for that long (the test bounds it at 7 round trips). That is
  about 0.1 s on a LAN and visible on a slow link. Rolling ahead through the sync would need ggrs
  changes.
- **A join is taken at the settled tick, not the current one.** The host steps back to its last
  tick with every seat's pads in (at most the rollback window), which is the same as any rollback.
  With only the host in, this is the current tick.
- **Joiners are not drawn in the host's shell.** The shell's seats are fixed when the ride loads.
  New seats ride in the simulation (and are hashed), but `main.rs` does not spawn their bikes on
  screen. That needs P76's local-join drawing, which is not in this branch. Filed as N12.
- **No P57 ghost** for joined riders, for the same reason.
- **A drop-in rides the host's bike set** whatever the guest picked (`drop_in_picks`). One ride
  loads one `.phy`, and that is the rule a lobby ride also has today.
- **Once opened, the ride's timing is the online one.** Pause no longer freezes the ride (others may
  be riding), and the tick schedule is the session's from the opening on, not the offline pacer.
- **Leaving an opened ride** (the host quitting) ends it for everyone, as any host leaving does.
  Handing the ride over or carrying on without the host is N11's.
- **`Peer` sends oversize datagrams over the stream.** Before this, `Peer` dropped them. The shell
  already did this; only the test and browser peer changed.
- **opentennis has no mid-match join to compare with**. I grepped its net code for drop-in and
  mid-session joins and found none, so this design is this port's own.

## Next

Nothing for N10. Follow-up filed as N12: draw seats that join an opened ride in the host's shell (needs P76).
