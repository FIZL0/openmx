# 1 — online feet down (FINAL)

Task V68. 2026-10-26.

## What I did

- The bug: V36's foot switch (`game_sim::foot_down::Feet`) lived on the drawn `game_render::rider::Body`
  and was stepped only by `Body::ride`. An online ride runs every seat's body machine inside the
  session's tick (`online.rs`'s `after` hook calls `Rider::body_frame`) and `Online::show` only calls
  `Body::posed_by`, so no online seat — remote or this machine's own — ever stepped the switch or
  set `Body::hold`: a stopped online rider kept both feet on the pegs.
- The switch's state moved into the rider: `game_sim::rider::Rider::feet`, stepped at the end of
  `Rider::body_frame` under the same condition `Body::ride` had (mode neither Wrecked nor Posed,
  `Input::of(ride, &body.veh)` after the body's step). Because it is in the `Sim`, a ggrs rollback
  (which saves/restores the `Sim`s by clone) takes it back with the rest of the seat, so a re-run tick
  steps it from the right state instead of twice. Stepping it in the online hook from a field kept
  beside the `Sim`s would have double-stepped every rolled-back tick.
- It is in the snapshot codec too (`snapshot.rs`, `wire!(Feet { held, fade, weight })`), so a resync
  or a mid-ride join carries it. 18 bytes a seat more on the wire's state. **Not hashed**: nothing in
  the simulation reads it, it is a drawing input, like `body_clocked`.
- `Body::held_by(&Rider)` (new) sets `hold` from the body's mode and copies `feet` off the rider.
  `Body::ride` now ends in it (its own stepping removed: same behaviour, the step simply runs one call
  earlier, inside `body_frame`), and `Online::show` calls it after `posed_by`.
- Knock-on: `game-sim/tests/platforms.rs`'s resync bytes re-pinned, 13391 → 13463 (18 bytes × four
  riders) and the new digest; the per-tick hashes and their digest did not move. `game-sim/tests/hash.rs`'s
  `size_of::<Sim>()` pin 11520 → 11536 (the field is not walked, like the rest of the body's machine).
- opentennis has no counterpart (no rider body); nothing taken.

## What the original does

No new reading: the switch is V36's (proved there against the body actor's switch, which S109 proved
against the original). The original runs the riding ragdoll for every rider; there is no "remote"
special case, which is what this restores.

## How it was proved

- `crates/openmx/tests/netplay.rs` `a_stopped_remote_riders_feet_come_down_on_the_local_ones_tick`:
  host + guest over a 20%-loss, 3-round-lag link (host reruns ticks), the guest rides 2 s of gas, brakes
  and stands; each seat's body frame run in the tick as `online.rs` does. On every settled tick the
  host's copy of the guest's seat has the guest's own `Feet` (and speed, yaw, mode), the feet come down
  on the same tick on both, and that tick is the one the guest's speed first drops under the stand
  speed with no yaw (V36's rule). Mutation check: with the step taken out of `body_frame` it fails
  ("no foot ever came down").
- `crates/openmx/tests/foot_down.rs` (disc): a second body posed the online way (`posed_by` +
  `held_by`) beside the one `Body::ride` poses, every tick of V36's ride/brake/stand/off: same feet,
  same holds, same palette. Mutation check: with `held_by` not copying the feet it fails on the first
  riding tick.
- `same_ride.rs`'s one-seat online-vs-offline test now compares the feet as well, in the codec's bytes.

## Not verified / not 1:1

- Not watched in a live two-machine shell ride: the proof is the headless session plus the disc
  body test; the draw call between them (`Online::show` → `held_by`) is two lines, read, not run.
- The feet are not in the online checksum: a peer whose feet parted would not be caught as a desync.
  Deliberate (drawing state, derived from hashed state every tick), not a gap in the switch.
- A snapshot saved before this change does not decode (the codec gained 18 bytes a rider); none are
  kept on disk by design, as far as I know — resyncs and joins are made fresh between same builds.

- Audit skipped at `./kit done`: its only findings were plan/DONE.md:121 and :392, other tasks'
  addresses; none in this task's files.

## Next

Nothing.
