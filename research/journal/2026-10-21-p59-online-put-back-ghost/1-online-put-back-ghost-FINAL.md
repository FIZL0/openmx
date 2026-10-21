# 1 — online put back ghost (FINAL)

Task P59. 2026-10-21.

## What I did

- **Started over from wip/P59, and kept less of it.** That attempt added an `online::Seats` game,
  meaning `Rides` plus one `vehicle::Ghost` per seat, because when it was written the ghost lived in
  the shell's `Garage`. S63 (37f1cfb) then moved the ghost into game-sim's `Rider`. It is now in
  `Sim`, cloned by `Rides::save`/`restore`, and its bytes go out with `game_sim::snapshot`. So the
  rollback state already carries it, and `Online::show` clones each seat's sim into the shell seat
  whose `sim.rider.ghost` the draw reads (main.rs, P57's alpha). The wip merge broke because it
  built against the `Garage` ghost that no longer exists. Most of the task had been done by S63;
  what was left:
- `net/rides.rs` `Rides::hash` folds each seat's ghost (clock and alpha bits, the flag) into the
  checksum peers compare. A one-player `Sim`'s walk leaves the ghost out on purpose (S63, to keep the
  one-player pins), and online every seat is a one-player `Sim`, so without this a peer whose ghost
  had drifted would never show up as a desync. The ghost can only drift if the ride drifts too (it
  is a function of the put-back tick and `dt`), but the checksum now says so directly. Only the
  online checksum changes: nothing pins its values, and every peer runs the same build (N7).
- Tests in `crates/openmx/tests/netplay.rs`:
  - `a_put_back_on_one_peer_is_a_ghost_on_both`: two machines over a lossy, lagged link (rollbacks
    run), with the host pressing L1+R1 for one tick every 120. Each machine records every seat's
    ghost after its last run of each tick, which is the state it drew. On all 366 settled ticks host
    and guest hold the same `Ghost` and the same `shown()` alpha bits, and the settled hashes agree.
    The put-backs land exactly on the ticks the chord was logged for (one tick after the press:
    the session's input delay). The fade matches a lone `Ghost` stepped on `rides::tick_dt`: half
    on the put-back's tick, rising, solid on the 91st tick after. The guest's bike is never a ghost.
  - `the_online_hash_and_bytes_carry_the_ghost`: two rides that differ only in seat 0's ghost have
    equal one-player `Sim::hash` and different `Rides::hash`, and `Rides::encode`/`decode` hands the
    ghost to another machine partway through the fade.
- From opentennis: nothing. It has no put-back fade.

## What the original does

Has no online play. The fade is P57's and S63's (`crates/game-sim/src/ghost.rs`); this task only
makes every peer agree on it.

## How it was proved

A bit-exact comparison across two simulated peers (netplay.rs harness, ggrs over the in-process lossy
link), 366 settled ticks, three put-backs, checked against an offline step of the same `Ghost`.

## Not verified / not 1:1

- Not run between two real `openmx` processes or over a real network. The harness drives the same
  `Netplay` + `Rides` the shell does, and `Online::show` → `seat.sim.rider.ghost.shown()` is the
  only shell code on the path. That code is unchanged and was not exercised by a test.
- Not looked at on screen online. Whether a networked ghost *looks* see-through depends on P57's
  draw, which reads the same field offline and online.
- The ghost's collision half (S63) is not exercised online: online each seat is its own one-player
  `Sim`, so the seats have no rivals and never collide or hold each other's clocks. That is how
  online rides are built today (R9), not something this task changes.

## Next

none
