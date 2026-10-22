# 1 — drop in on the plane

Task N19. 2026-10-22.

## What I did

- **The host puts a joiner's plane into the joiner's spawn.**
  - The host's `Online` takes a peer on in `Online::receive` (`Event::Arrived`). It now also reads the
    joiner's pick's `machine` (N18's `openmx_net::Pick::machine`, set by `Lobby::set_machine` from
    the ride setup or `--vehicle biplane`).
  - When that machine is `biplane`, the spawn the host sends is `rides::spawn_flying(first, spawn,
    template, bays[seat])`. This is new: it decodes the spawn onto the first seat's constants, puts a
    copy of the ride's plane in the seat's lane with N18's `vehicle::start_lane` (standing in the
    seat's own bay), and encodes it again with `seat_bytes`.
  - It goes on top of the saved markers (`spawn_marked`, P105), so a joiner brings both.
  - The state every machine is sent at the join carries the flight through the snapshot codec, which
    has carried flying seats since N17. So every machine seats the joiner flying from the same place.
    `Rides::seat`'s `probe` puts the ray probe back from the ride's own plane, as for any flying
    seat.
- **Why the state, not a flag in `spawn_on`.** The brief said `spawn_on` carries presets only. A
  per-seat machine flag there would make `Rides::seat` place the plane itself, and it would need the
  bays. `Rides` is built in 27 places, and none of them has the bays. Putting the plane in the state
  on the host, as P105 does with the markers, changes no format and no `Rides` field.
- **The host of an opened ride now has the bays.** "Open to Online" in `main.rs` calls
  `on.bays(online_bays.clone())` beside `on.planes`, so the host has the bays a joiner's seat stands
  in. `online_bays` is cloned at the start too.
- **The refusal is gone.** A drop-in no longer refuses a player whose setup picked the plane, or
  `--vehicle`. It still refuses a tape and `--players`. The joiner's garage follows its session seat
  (N17's `Garage::follow`) once the host's state arrives.
- **What falls back to the bike, with a console line:**
  - a joiner with more than one seat (each seat would need its own bay, and `spawn_on` has one state
    for them all; the shell's drop-in has one local player anyway);
  - a ride with no plane, or no bay for the seat.
  - Every machine does the same, because only the host decides and every machine is sent its state.

From opentennis: nothing applicable (one machine a player there, no vehicles).

## What the original does

The original has no online play. A start on the plane is offline P41, the player standing in their
bay, which N18 matched with `vehicle::start_lane`. This task reuses that start for a joiner.

## How it was proved

- **tests/lobby.rs `a_drop_in_whose_setup_picked_the_plane_rides_in_flying`.**
  - Setup: real `openmx_net` LAN networks, the disc's `biplane.phy`. The host opens a freeride of one
    with `Online::open`, with `planes` and `bays`, as the shell does. The guest joins by the room code
    with a pick whose machine is `biplane`, and rides in with `Online::drop_in`.
  - Once the guest is seated, both machines have the guest's seat flying from its bay (`bays[1]`) and
    the host's seat on its bike.
  - Four more seconds: the last 30 settled hashes are equal on both machines, with 0 desyncs, and the
    guest is still flying.
  - **It tests the change:** with `flying` forced to `None` in `Online::receive`, the test failed at
    the arrival assert. The log showed "rides in on the biplane", and the guest joined at tick 43.
- **tests/netplay.rs `a_joiner_that_picked_the_plane_is_seated_flying_from_its_bay`.**
  - `spawn_flying` → `spawn_on` → `Game::seat` gives the seat exactly (`==` on the `Flight`) the lane
    `start_lane` puts in the bay. N18 holds that lane to the offline shell's plane start, tick for
    tick.
  - The seat keeps its own preset (3) and its saved markers. A second joiner without the plane comes
    in on its bike, and the host's seat is untouched.
- `./kit check`: see the commit (the gate `./kit done` runs).

## Not verified / not 1:1

- **The whole feature is the port's own.** The original has no online play. It is checked only
  against the port's own start on the plane (N18's `start_lane`, held to the offline shell there).
- **No windowed shell dropped in through the menus with the setup's machine row set to the plane.**
  There is no display automation for two windows here. The path from the shell's `set_machine` to the
  pick is N18's and unchanged. The lobby test sets the pick directly and uses `Online::drop_in`, but
  not the shell's own branch in `main.rs`. The shell's garage starting on the plane before the host's
  state comes was not watched either.
- **The bays in the tests are made up on a flat world, not parked on a track.** The shell's host
  parks `online_bays` exactly as N18's lobby ride does.
- **A joiner with two or more seats always rides in on the bike**, even if its pick says the plane
  (a console line on the host). The shell's drop-in refuses `--players` anyway, so only a
  non-shell client could hit it.
- **`peer.rs`'s headless peer (wasm/tests) has no plane at all**, so it still seats every joiner on a
  bike. That is unchanged: its ride is a flat test ride with `plane: None`.
- **The joiner's machine is not in the handshake.** As in N18, it relies on the host reading the
  joiner's pick from the room. Only the host reads it, and every machine gets the result in the
  state, so a mismatch cannot desync.

## Next

Nothing for this task.
