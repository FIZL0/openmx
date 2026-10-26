# 1 — online peers ridden in their looks (FINAL)

Task N16. 2026-10-26.

## What I did

- **The bug.** `ride()` sized `world.seats` from `--seats`/`args.players`. The `--online --peer` path
  passed `--seats N`, so every seat was drawn there. But a ride started from the lobby (P86), a drop-in
  (N10) and a freeride opened online (pause → Open to Online) all loaded **one** shell seat. Online
  copied every peer's sim into `Online::show(d, …)` for `d < world.seats.len()`, which was only seat 0,
  so peers rode in the session and were never drawn on anyone's screen.
- **Lobby start** (`crates/openmx/src/main.rs`, `crates/openmx/src/peer_looks.rs`): the ride now loads
  a shell seat for every room seat. Each one is dressed from the room seat's pick with
  `peer_looks::lobby_others`, in `Online::drawn()` order (this machine first, then the others in seat
  order), as split screen dresses its extra players (`args.others` → rigs with name texture, plates and
  repainted body). Open seats are loaded in this machine's look and are not drawn while nobody sits in
  them. Cameras, HUD seats and the pause menu still count only the local players (`args.players`), so a
  peer's seat gets no viewport and the local machine settings are not indexed past them.
- **Mid-ride** (drop-in, open-to-online, a seat changing hands): every online tick the shell grows
  `world.seats` to `Online::seat_of().0` and seats each new one in this machine's look. Then
  `peer_looks::Dressed` checks each seat's current room pick (`Online::seat_pick(d)`, new) against what
  it is dressed in, keyed by peer, rider, model and look. A seat whose key changed is dressed again
  (`dress_peer`, new, the same steps as the load's), its models are uploaded to the GPU, and its HUD tag
  is renamed.
- **The bike model travels**: `openmx_net::room::Pick` gets `model` (`#[serde(default)]`, so an older
  build's pick still parses and draws as the receiver's own model). The lobby and Open-to-Online set it
  from `args.bike`. It is looks only: the simulation still rides the room's bike set.
- `Online` now keeps the room it was made from in `lobby()` and `drop_in()` too, so `seat_pick` works
  from the first tick and not only after the next `Event::Room`.
- From opentennis (`hst-base/src/mods_online.rs`) I took the idea that a look the receiver cannot make
  falls back to a stand-in and never touches the match. Here a missing model or rider falls back to
  this machine's own (`garage_pick`), and nothing in `peer_looks` reaches the sim.

## What the original does

Not applicable: the original has no online play. The look of each rider (rider archive, outfit pieces,
name, plate number, bike model) is the garage's, as split screen already draws it.

## How it was proved

Two tests in `crates/openmx/tests/lobby.rs`, each with two real LAN nets in one process:

- `a_lobby_ride_draws_each_peer_in_their_own_look`: host and guest each pick a different rider, look
  and bike model, ready up and Start. On each machine, `lobby_others` puts the other player at shell seat
  1 in their rider, bike model, outfit, name and number. After 60 frames of `Online::lobby`, shell seat 1
  is `show()`n (sat in, drawn), an open seat is not, `seat_pick(1)` is the other peer with their look,
  and `Dressed::lobby(...).changed()` is empty (the load dressed everything).
- `a_peer_who_drops_in_is_drawn_in_their_own_look`: a freeride opened online, and a guest joins it
  with its own look. Both sessions grow to two seats. On each machine `Dressed::default().changed()`
  names seat 1, `garage_pick` gives the other's rider, model and look, and once dressed the seat is not
  dressed again.

`./kit check -- -p openmx --test lobby`: 43 passed. Gate: `./kit check --all` at `./kit done`.

## Not verified / not 1:1

- **No by-eye or shell-level run of a lobby ride.** The shell has no command-line way into the lobby, and
  two windowed shells on this desktop cannot be driven through menus (no keystrokes). The tests prove the
  data path (room pick → shell seat pick → sat in and drawn), not pixels. The rig building in
  `dress_peer` is the same steps as the split-screen load's, but no test runs it.
- **`--online --peer` (the UDP path) still draws peers in this machine's look.** It has no room and so
  no picks (`seat_pick` → None). Peers were already drawn there, so it was not the bug. Not changed.
- **Dressing mid-ride loads assets on the ride thread**: rider and bike archives, repaint, and a GPU
  upload in one tick. That may hitch a frame when someone drops in. Not measured.
- **Dressing a seat again leaks the old rig's models/textures** in `assets` (new `@peer{n}` names each
  time). This only happens when a seat changes hands or a look changes, which is rare. Not freed.
- **A peer's local garage choice vs. its lobby pick**: the lobby pick's rider is `args.rider` and the
  model is `args.bike` at the time of the lobby. If a player changes garage after joining, peers see the
  pick that was sent.
- A peer whose bike model or rider this machine does not have draws this machine's own instead (the
  same as split screen's missing-archive fallback). No test for a missing archive.

## Next

Nothing for N16.
