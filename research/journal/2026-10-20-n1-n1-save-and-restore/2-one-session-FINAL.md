# 2 — one session (FINAL)

Task N1, second session. 2026-10-20. S54 has landed (`Sim::others`, `add_player`, `step_all`), so the
stand-in of part 1 — four independent `Sim`s — is replaced by S54's ride. The S54 merge had already
taught the codec about `others` (`snapshot::encode` writes the other players' riders when there are
any, `decode` refuses a player count that differs from this machine's), so what was left was the
tests, one constant and the vectors.

## What I did

- **`Player::phy` is `Arc<Phy>`** (crates/game-sim/src/lib.rs). S54 made it a plain `Phy`, so every
  snapshot of a session copied each other player's bike numbers (~1.3 KB each); now it is shared by
  every clone like `Sim::phy` and the tables, as the brief asks of constants. `add_player` still takes
  a `Phy` and wraps it, so no caller changes; nothing outside lib.rs builds a `Player`.
- **`tests/players/mod.rs` is one `Sim`**: player 1 is `Sim::ride`, players 2–4 `add_player`, same
  seeds-of-behaviour as before (wrecks on for players 2 and 3, the four scripted pads). They now ride
  `step_all` in player order and can reach one another (S54's joust). Pads: players 1 and 2 ride the
  `Sim`'s own two pads through `poll_all`, so the input layer's timers and press edges are state the
  snapshot must carry; players 3 and 4 have no pad on this machine (`input::PADS` is 2), as a peer's
  riders do not, and their `Controls` come from `hands(input)` — the frame's `Input` read through a
  fresh connected `Manager`, a pure function of the frame, so no state lives outside the `Sim`. The
  consequence: for a pad-less rider a held button is a pressed one (the reset chord fires on every frame
  it is held). `whole` now includes `others`.
- **`tests/snapshot.rs`** (10 tests) over the one session: save/step/restore/step at six points by hash,
  `Snapshot::hash` and the whole `Debug` text; the hash-skipped idle pool, per player, is seen by the
  text and carried by the bytes; a late input rolled back over is the on-time run, now for a rider on
  one of the `Sim`'s pads (player 2) **and** a pad-less one (player 3), with an assert that each guess
  really was wrong (at tick 140, the first choice, player 2's pad was steady, so "held" was right and
  proved nothing — moved to 170, where its reset chord begins); the snapshot shares every player's
  `phy` `Arc`; encode → decode onto the same session somewhere else in its ride gives the same hash,
  text and re-encoded bytes and the two ride on together for 40 ticks; decode keeps local stunt sets
  (players 1 and 4) and every `Arc`; broken bytes are refused untouched, now including a session one
  player short and a one-player state. The ggrs 0.13 `SyncTestSession` transcription runs over the one
  `Sim` (cells hold one `Snapshot`, checksum `Sim::hash`): no mismatch at check distances 1–7 over 320
  frames, its last state equal (hash and text) to a plain run of the same pads two frames late, and the
  unsaved-counter control still caught.
- **`tests/platforms.rs`**: `riders_run` steps the one session and rolls back over guesses for players 2
  and 4 (one with a pad, one without); `riders_wire` is one `encode`. `RIDERS_*` re-pinned with
  `print_the_riders_vectors` (spliced: only the seven consts moved). The wire length is 11 827 bytes
  for the session, against 16 862 for four separate `Sim`s — three fewer input layers, magics, tick
  counters and seeds.

## How it was proved

- `./kit check`: TESTS PASSED. One earlier full run failed in `openmx --test netplay`
  (`four_peers_and_eight_riders_on_a_bad_network`: "k waited 62 rounds", bound 60) while three other
  slots' check runs were going; it passed alone (13 tests) and in the next full run. N1 changes nothing
  that crate's netplay steps (it holds a `Vec<Sim>`, one per seat, and never calls `add_player`), so I
  read it as a timing bound under load, not a regression.
- `crates/game-sim/tests/cross/platforms.sh`, exit 0: **aarch64-unknown-linux-musl ok** and
  **wasm32-wasip1 ok**, with snapshot's 10 tests and platforms' 12 passing on each — so the re-pinned
  `RIDERS_*` (hashes every 32 ticks, the digest of all 256, the rolled-back run equal to the straight
  one, the resync bytes) agree on x86-64, aarch64 and wasm32.
- Every single-player hash and vector is unchanged (no edit to the step; `Player::phy`'s type is not
  walked).

## Not verified / not 1:1

- **Not ggrs itself**, as in part 1: game-sim may take no registry dependency (its own platforms test
  forbids one), so the sync test is ggrs 0.13's `SyncTestSession` transcribed. N2 vendors ggrs in
  crates/openmx.
- **Players 3 and 4's controls are not through the `Sim`'s input layer**: it has two pads. Their hands
  are a stateless read of each frame's input, so a held button is a press every frame. That is a choice
  of the test script, not something the original says; it is how a pad-less (remote) rider's controls
  have to arrive at `step_all` anyway. The shell's online mode (N2) still runs one `Sim` per seat and
  is not moved onto one session here — outside N1's files.
- **The riders' cost was not re-measured** for the one session; part 1's figures (save+restore of four
  separate `Sim`s 8.1 µs, hash 19 µs) are for the stand-in. With `Player::phy` shared the copy is
  smaller, not larger, but no number backs that.
- **The rider's disc constants** (stunt set, clips, wreck files, body numbers) are still copied per
  snapshot, for every player — as in part 1, moving them behind an `Arc` reaches far into rider.rs.
- **The world and the camera are not snapshotted**, as in part 1: nothing on a track moves, and the
  camera is the shell's.
- **The whole-state compare is `Debug` text** (as in part 1); `Player` derives `Debug`, so it covers the
  other players' every field.
- Nothing here is compared against the original: it has no rollback.
