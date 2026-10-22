# 1 — start an online ride on the plane

Task N18. 2026-10-22.

## What I did

- **A seat that starts on the plane has it in its `Sim::plane` lane before tick 0, on every machine.**
  - Where: `vehicle::start_lane`. It puts a copy of the ride's plane template (N17's
    `rides::plane_on`) into the lane, standing in the seat's own bay.
  - Why it matches offline: offline, a player whose setup picked the plane starts in it, standing in
    their bay (P41, `Lot::plane`), and its first step places it there. Online it is placed before the
    first tick, so the first frame starts from the same flight.
  - The bay is seat `s`'s copy in the lot. The shell parks the bays for `net::SEATS` copies with
    `vehicle::park`. A copy's place depends only on the copies before it, so seat `s` gets the same
    bay whatever the seat count, and every machine gets the same bays from the same disc.
  - `Online::start_planes(flies)` does this per seat. It refuses after tick 0. A ride with no plane,
    or no bay for a seat, starts that seat on the bike and prints a line, the same on every machine.
- **Every machine needs to know every seat's pick before tick 0.**
  - **Lobby:** `openmx_net::Pick` has a new field, `machine`: `biplane`, or empty for the bike.
    - It is `#[serde(default)]`, so an older build's pick reads as the bike.
    - It is not in the handshake, for the same reason the markers are not.
    - The shell sets it every frame with `Lobby::set_machine`, from the ride setup's machine, or
      `--vehicle biplane` over it. It goes out on the same terms as the preset (N13).
    - At the start, `Online::picked_planes(&room)` turns the picks into `start_planes`.
  - **`--online` / `--peer`:** the card byte each machine sends ahead of every datagram (N15) now
    carries a flag, `udp::PLANE` (bit 7). `Udp::presets` masks it off, and `Udp::planes` reads it.
    - `Online::start` has a new `plane` argument for this machine's first seat. The shell passes
      `--vehicle biplane` there.
    - `take_presets` starts the flagged seats once every peer's card is in. That is already before the
      first tick, which is where N15 sets the presets.
- **The refusals are gone.** `--online` no longer refuses `--vehicle`, and a lobby ride no longer
  refuses the setup's plane.
  - A drop-in still refuses a player on the plane (see below).
- **Shell:** before `--peer`s' cards are in, the garage does not follow the session's lane
  (`Online::machines_known`). Without that, a seat starting on the plane flicked to the bike and back,
  and the camera snapped twice.

From opentennis: nothing applicable (one machine a player there).

## What the original does

The original has no online play. Starting a freeride on the plane is offline P41, which this follows
(the plane in the player's bay). There is nothing in PCSX2 to compare against.

## How it was proved

- **tests/netplay.rs `a_seat_starts_the_ride_in_the_plane_on_every_machine`.**
  - Setup: two peers on a link with 5% loss and 3 rounds of lag. g's seat starts in the plane in its
    bay on both machines, and the host's seat on its bike. g asks for the bike from tick 300. The plane
    is the disc's real `biplane.phy`.
  - The two machines agree on every settled tick's hash and pads from tick 0 (>500 ticks), with no
    desyncs.
  - The same pads ridden offline give the same state before every tick.
  - **The online lane equals the offline shell.** For every tick g flies, the online lane's `Flight`
    equals (`==`, every field) the flight of an offline `Garage` started on the plane in the same bay.
    That garage is stepped the way `Seat::pass` steps it (poll, then `Garage::step`). This holds over
    all 301 ticks from tick 0.
  - g is back on its bike at tick 301, one tick after asking (the input delay).
  - **It really ran with the disc:** I made one expectation off by one on purpose, and the test failed
    with 301 vs 302.
- **tests/lobby.rs `a_lobby_pick_of_the_plane_starts_its_seat_in_it_on_every_machine`.**
  - Setup: two real `openmx_net` networks on the LAN. The guest's menu sets `set_machine("biplane")`,
    and the pick reaches the host's room.
  - Both machines read the same `picked_planes`. Each builds `Online::lobby`, then
    `planes`/`bays`/`start_planes`, as the shell does.
  - Before the first frame the guest's seat is in the plane at its bay on both machines, and the
    host's seat is on its bike.
  - Over four seconds (109 settled ticks) the hashes are equal with 0 desyncs. The guest is still
    flying at the end.
  - It ran with the disc: `--nocapture` printed "1 seat starts in the biplane" on both machines.
- **`udp.rs` `every_seat_s_starting_machine_is_known_once_every_peer_is_heard`.** The flag reaches
  the other machine, both machines read the same seats in the plane, and the presets come out with
  the flag masked off.
- **Two headless shells** over localhost UDP (debug build, the disc's free06): `--online … --seats
  host,g`, with g given `--vehicle biplane`.
  - Both printed "1 seat starts in the biplane", 592 ticks, 0 desyncs, and the same `state before tick
    300 0xb4184bc1b84ea6a0`.
  - The same run without `--vehicle` gives `0xabe9fdbe77ecc288` (N17's bike-only hash). So the plane
    really is in the shared state.
- `./kit check`: TESTS PASSED (full gate).

## Not verified / not 1:1

- **The whole feature is the port's own.** The original has no online play. It is checked only
  against the port's own offline start on the plane.
- **The offline equality was proved on a flat test world with a made-up bay, not on a track.** On a
  track, the online bays are `park(…, net::SEATS)` and offline they are `Lot::new(…, players)`. These
  are the same function, and copy `s` does not depend on the count, but no test compares them on a
  disc track.
- **The headless shells' pads are idle**, so the plane was started there but not flown. The flight
  online is proved by the netplay test, which uses mashed pads.
- **No real windowed shell went through the lobby with the setup's machine row set to the plane.**
  The menu → `set_machine` → pick path in the shell loop is not exercised (no display automation for
  two windows here). The lobby test sets the pick directly.
- **A drop-in on an opened ride still refuses a player whose setup picked the plane**, with "drop
  --vehicle". The joiner's seat is spawned by the host from the ride's start (`rides::spawn_on`), and
  that spawn carries no machine yet. Filed as a follow-up.
- **The host starting "Open to Online" while flying** is unchanged from N17: the flight goes in as it
  is.
- **The machine pick is not in the handshake.** Like the markers, it relies on every machine reading
  the host's room. A guest on an older build would read the seat as on the bike and desync. The build
  id check is what prevents that, not the handshake.
- **A seat whose pick says plane on a track with no plane, or seat 33+**, starts on the bike with a
  console line. That is the same on every machine but untested.
- **Seats 2–4 on one `--peer` machine** have no pad (P35), and their card byte never carries the
  plane, so only a machine's first seat can start on it.
