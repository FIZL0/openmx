# 1 — session markers (FINAL)

Task P88. 2026-10-21.

## What I did

- `crates/game-sim/src/markers.rs` (new): five marker slots a player and a selection (`Markers`, a
  new `Sim::markers` field). Each d-pad direction fires once, on the tick its input-layer held timer
  crosses 250 ms (`prev_held_ms < 250 <= held_ms`), so a hold needs no state of its own and a tap
  never fires. At the 30 Hz clamp the timer counts 33, 66 … 231, 264, which means a hold fires on the
  eighth tick after the press, and 200 ms (and 231) never fires.
  - **Down** keeps a clone of the whole `Ride` in the selected slot: position, facing, speed and
    spin. I kept the speed. A stopped marker means a trick has to be ridden up to again, and keeping
    the speed makes the next tick after a teleport bit-for-bit the tick after the placement
    (tested). Refused while wrecked or crashed, and refused on a player.
  - **Up** teleports back to the slot's ride (`Rider::teleport`), or to the selected player's spot
    (`Rider::teleport_to`: their bike's origin, forward and up, or their put-back record while they
    are down). A player's `Ride` is not copied because their bike can be a different machine. You
    are put down stopped, the way a put-back puts you down.
  - **Left/Right** step through slots 1–5, then each other player in seat order (yourself left
    out), then wrap round.
  - Both teleports go through `teleported()`. It does what a put-back does: respawn out of a
    wreck, clear crash, ragdoll, tricks and the wreck page, then `ghost.start()` (P57 poof, S63
    pass-through).
- The d-pad (P73's table) keeps everything the original gives it in a ride: commands 0–3 are the
  digital stick (lean, steer, rider fore/aft) and the direction half of a trick gesture. Markers take
  nothing from the bike. A hold with a trick button down is a gesture, so no marker fires (tested:
  the ride hashes as if markers did not exist). Biplane rides (`sim.plane`) have no markers.
- The teleport runs **after** the tick's step, so the next tick starts from exactly the kept state.
  It is called in the shell's `Seat::pass` and in `net::rides::Rides::step` (online and rollback)
  with every seat's `Spot` taken before anybody's tick. That makes it pad input on agreed ticks:
  rollback re-runs it from the confirmed pads, and the state lives in `Sim`, so snapshots carry it.
- Hash and wire: `Sim`'s walk adds `markers` only when they are not fresh, and the snapshot writes a
  tagged `P88marks` section only then. A session that never touches them hashes and encodes exactly
  as before, so every pinned hash and byte vector stands. Decoding peeks at the tag. `Sim`'s size
  pin moved 10544 → 10592.
- HUD (`hud.rs`): `MarkerBanner` shows for 2 s when the selection or a slot's fill changes, and the
  whole time a d-pad direction is held. It is a top-centre panel with five boxes (filled = holds a
  marker, ring = selected), a label ("marker 3 set", "marker 2 empty", "to NAME", or "to P2" with
  no name), and a hold-progress bar that fills over the 250 ms. Online, names come from the lobby
  lineup (`Online::names`).
- World: the GPU path draws each placed marker as the rider's own bike model at alpha 0.2, only in
  that rider's own view (a per-view actor list).
- Tests: `crates/openmx/tests/markers.rs` has 8 tests:
  - Down held 250 ms places once;
  - 0/100/200/231 ms taps do not;
  - a trick gesture is not a marker;
  - Up lands on the marker bit-exact, ghosted with the full 91-tick fade;
  - the tick after a teleport repeats the tick after the placement;
  - Left/Right reach all 5 slots, then both other players, and wrap;
  - a teleport onto a moving rider collides with nothing (the control check shows the same two
    bodies do hit when not ghosted);
  - snapshots and hashes carry the markers, and cost nothing while untouched.
- opentennis has nothing like this to take from: its `markers.rs` is the player tags shown over
  heads before a serve.

## What the original does

Nothing. The original has no markers or teleports. Its only part here is the d-pad's meaning in a
ride (P73: commands 0–3), and that is all kept.

## How it was proved

Unit/integration tests only (`crates/openmx/tests/markers.rs`, 8 tests on the stand-in bike on a
flat floor at 30 Hz), plus the full `./kit check` gate passing. No PINE capture: there is nothing in
the original to compare against.

## Not verified / not 1:1

- **Nothing to be 1:1 with.** The feature is the port's own, and its numbers (5 slots, 250 ms, 2 s
  banner, 0.2 marker alpha) are the brief's or mine.
- **HUD banner and world marker ghosts not looked at.** I ran no shell window and took no
  screenshot. The layout, the label text and the faint bike are untested by eye.
- **The software renderer draws no marker ghosts.** It has no alpha. It draws the HUD banner only.
- **A d-pad hold still leans/steers the bike** for the 250 ms before it fires, and after. This is
  on purpose (the original's binding is kept), but a rider placing a marker mid-ride will feel it.
- **Online not run live.** Markers go through `Rides::step` and the snapshot, and the tests cover
  determinism, hash and encode/decode, but no two-shell session was ridden. In an online session,
  empty or absent seats in `sims` still count as players in the selection. They show as "P<n>" and
  teleport to wherever that sim's rider stands.
- **Shell local multi-seat:** each local seat is its own `Sim`, so markers go between seats through
  `Spot`s and seats never collide anyway. The ghost there only matters for the scenery/bodies it
  already covers.
- **Teleporting onto a player puts you down stopped**, not at their speed. Their machine can differ
  from yours, so their `Ride` cannot be copied.

## Next

Nothing for this task. If the human wants it: look at the banner and the faint markers in a shell
window, and decide whether the d-pad's lean during a hold needs a separate binding.
