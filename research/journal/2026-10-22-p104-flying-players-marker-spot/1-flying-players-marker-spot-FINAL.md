# 1 — flying players marker spot (FINAL)

Task P104. 2026-10-22.

## What I did

- **`Garage::spot(sim, world)`** (crates/openmx/src/vehicle.rs) is where a player is for a marker's Up onto them.
  - With the bike up, it is `markers::Spot::of(&rider)` as before (the bike, or its put-back spot while down).
  - With the plane up, it is the ground under the plane, upright, along the plane's flattened heading. That is
    exactly where P102's switch back to the bike stands it. Both now go through one helper, `under`.
  - If there is no ground under the plane, it is the plane's own spot (its origin, nose and up), and the bike falls
    from there.
- The shell's pass (main.rs `simulate`) builds the seats' spots with `garage.spot` instead of `Spot::of(&rider)`.
- `markers::Spot::of` in game-sim is unchanged, because a `Sim` holds no shell plane. Online `net/rides.rs` is
  unchanged too, because online rides are bike only (P102, N17).

Why the ground under the plane and not the plane's position: teleporting a bike to a plane at altitude drops it
hundreds of feet into a wreck. Landing it where the switch would land it means "go to that player" behaves the
same whichever machine they are on.

From opentennis: nothing applicable (no machines, no markers).

## What the original does

Nothing. Markers (P88) and the spawn-on-the-player switch (P102) are the port's own.

## How it was proved

- Unit test `vehicle::tests::a_flying_players_spot_is_under_their_plane_not_at_the_bike_they_left` (Flat floor):
  - On the bike, the spot equals `Spot::of`.
  - After the switch to the plane, with the plane moved to (-30, 400, 70) nose up and pointing -z, the spot is
    (-30, 0, 70), level along -z and upright, and it is not the bike's old x.
  - A second rider `teleport_to` that spot ends at the same position as the player's own switch back to the bike.
  - It ran: filtered `./kit check` reported 1 test passed. Before the change the spot would have been the bike's
    origin, so the x assertion would fail.
- `./kit check` (full gate) via `./kit done`.

## Not verified / not 1:1

- **The port's own behaviour**, with no original to compare against (see above).
- **Not exercised in a window or with a tape.** Only the unit test covers it; no split-screen ride had one seat
  flying while another pressed Up.
- **Online is untouched.** `net/rides.rs` still uses `Spot::of`, which is right only while online rides stay
  bike only (N17). When N17 carries the plane in the session state, the online spots must use the plane too.
- **No-ground fallback is untested.** It puts the bike at the plane's spot in the air. It was chosen over refusing
  the teleport because `Spot` lists have no "absent" entry, and it is not covered by a test.

## Next

Nothing: the task lands.
