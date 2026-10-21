# 1 — seat presets online (FINAL)

Task N13. 2026-10-21.

## What I did

Every online seat now rides its own player's controller preset (P79) instead of preset 0.

- **The room carries it.** `openmx_net::Pick` has `preset: u8` (`#[serde(default)]`, so a pick from an
  older build reads as 0). The shell keeps this machine's pick in step with 1P's preset every front-end
  frame (`Lobby::preset`, fed `seats.preset(args.pad_seat(0))`, the same seat and wiring the ride's
  `set_presets` reads). An open lobby gets a `Cmd::Picks`. A change during a ride waits (`Lobby::repick`)
  and goes out once the lobby is back. Host and Join carry the pick as it is.
- **The handshake carries it.** `Handshake::presets` holds every seated player's preset in seat order. A
  joiner whose list differs from the host's is refused with "rider N's controller preset differs (host: …,
  you: …)", the same way bikes are refused. The lists come from the same room, so a difference means a
  stale line-up.
- **The ride is built on it.** On a lobby ride (`main.rs`) each seat's `Sim` gets
  `set_presets([pick.preset, 0])`, and an open seat gets 0. `peer.rs`'s `sims(room)` does the same. Open
  to Online (N10) puts the preset the host's seat is riding into the host's pick
  (`net::rides::preset(&sim)`).
- **The ride's bytes carry it.** `Sim::bindings` is a constant that is copied but not encoded. Before this
  change, a seat that a join or a decode added was a clone of seat 0, so it picked up the host's table.
  `Rides::encode` now writes each seat as `seat_bytes`: one preset byte, then the snapshot. `decode`
  rebuilds each seat's table from that byte. A resync, a restart or a join therefore leaves every
  machine with the same table for each seat.
- **A joiner's spawn carries it.** `Rides::seat` takes `spawn_on(seat_bytes(start), &[(seat, preset)…])`.
  The shell's `Online` and `peer.rs` read each arriving peer's seats' presets from the room they last
  heard. The network seats a joiner, and says so in an `Event::Room`, before its `Arrived`. A peer with
  several seats gets a preset for each one.
- The per-frame line in the ride loop no longer forces preset 0 online. The drawn seat is a copy of the
  online seat anyway.

Taken from opentennis: nothing. Its handshake (`hst-base/src/handshake.rs`) has no per-player control
setting to copy. The shape (a field per seat, compared like the bikes) follows this crate's own handshake.

## What the original does

The original builds its binding table on the profile's preset when a ride starts (P73/P79). It has no
online play on PS2 (the online here is ours), so there is nothing in it to compare the wire against.

## How it was proved

- `openmx-net` `handshake::tests`: each mismatch is refused with its reason, `Handshake::of` reads the
  room's presets in seat order, and a pick serialised without `preset` reads back as 0.
- `tests/netplay.rs` `every_seat_rides_its_own_preset_and_says_it_in_bytes`: seats on presets 1/0/2
  decode onto seats built on preset 0 (and a grown third seat), keep their presets, and hash and ride the
  same afterwards. The same state and pads with every seat on preset 0 is a different ride for seat 1
  (preset 1's Square is not a brake) and the same ride for seat 2, which was already on 0. So the preset
  really changes the simulation. A joiner's seats spawn on the presets in their list, and a seat missing
  from the list falls back to the spawn's own. A preset of 4 is refused.
- `a_freeride_opened_online_takes_two_peers_mid_ride` (lossy simulated link): host on 3, g on 2, k's two
  seats on 1 and 0. Every machine ends with presets `[3, 2, 1, 0]`, and there are no desyncs.
- `a_freeride_opened_on_the_lan_takes_two_peers_mid_ride`: the same through the real `openmx_net` LAN
  lobby, handshake, room picks and drop-in. Every peer ends with `[3, 2, 1, 0]`, and the settled ticks
  agree.
- `tests/lobby.rs` `the_players_controller_preset_rides_in_the_pick`: Join carries the preset, a change
  in an open lobby is a single `Picks`, a ride under way holds the change, and the lobby hears it once it
  is back.

## Not verified / not 1:1

- **`--online` with `--peer`/`--seats` (the UDP path, N2) still rides preset 0 for every seat.** That
  path has no room and no handshake to carry a preset. It prints "not sent" as before. Fixing it means a
  per-seat flag on the command line or a UDP hello, which this task did not need.
- **No shell run with two real windows riding different presets.** The proof is the session tests on
  `Rides` (the online ride's code), not an `openmx` binary on a disc. The `main.rs` lobby-path lines
  (`set_presets` per seat, the mismatch println, the per-frame `Lobby::preset` call) are covered only by
  compiling and by reading.
- **A preset change during a ride does not reach that ride.** This is by design: the table is a constant
  of the ride, as in the original. The new preset goes out to the lobby afterwards.
- A peer whose seat is missing from the room `Online` last heard (it should not happen, because the
  network seats a joiner before it arrives) is spawned on preset 0, with a printed line.

## Next

Nothing for N13.
