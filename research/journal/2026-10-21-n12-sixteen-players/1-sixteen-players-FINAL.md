# 1 — sixteen players (FINAL)

Task N12. 2026-10-21.

## What I did

- **One cap, 32, runtime counts under it.** The ride's seat count was already a runtime number
  (`Lineup::seats`, `Room::seats`, `Rides::sims` are all `Vec`s); the caps were the constants.
  `openmx_net::SEATS` (room.rs) is now the one cap, 32, and `net::SEATS` is defined as it, so the
  rollback, the lobby and the state wire (`Rides::decode`/`seat`, `admit`) can never disagree. Its doc
  says what really limits a ride (below).
- **The local seat counts stay** as the brief asks: `pad::SEATS` and `menu::SEATS`/`SEATS_MAX` (8, the
  split screen's and the pads') and `roster::SEATS` (4, the join screen's and garage's columns) are
  this machine's seats, not the ride's; nothing online indexes them (checked: `online.rs` sizes its
  poses by `net::SEATS`, `Mods` clamps a seat past its rows, the overlay's radar loops over whatever
  riders it is handed).
- **The room** (`openmx-net/src/room.rs`): `Room::seat` is `take` with the reason it can't — "No players
  to seat.", "The lobby is full (0 seats open, 1 wanted; 16 seats).", and at the cap "...32 seats, the
  most a ride has)." — and the host's guest handling now refuses with that reason. `Room::resize(n)`:
  2..=SEATS, never under the last taken seat, with a reason either way. `Cmd::Seats(n)` asks the host's
  network task for it (refused while a ride is under way, as Track and Wrecks are).
- **The lobby** (`menu.rs`): the host's rows have **Seats** after Private (left/right/Cross, round
  from the cap to two); the pick is `Settings::lobby_seats` (`lobby_seats = n` in settings.txt, written
  only when it is not the usual 4, so older files read and write as they were), which `room_for` and
  the pause's Open to Online (main.rs) open their room with. The seat panel lists up to six seats a
  line each as before; a bigger room's go in columns of eight short lines ("12 Ridernam R").
- **Tests**: `tests/netplay.rs` `sixteen_riders_on_four_peers` (16 seats dealt round four machines on
  the real `Rides`, 5 % loss, 100 ms lag, 360 ticks: every settled tick the same pads and state on all
  four, no ggrs desync, every seat a state of its own) and `sixteen_machines_measured` (`--ignored`, the
  numbers below); the line-up test now takes 32 and refuses 33 with the cap in its reason. room.rs
  `a_room_takes_sixteen_and_refuses_past_the_cap`. lobby.rs's host walk goes through the Seats row; the
  settings round trip carries `lobby_seats: 16`. The harness's `Run` counts every machine's datagrams
  and bytes (`sent`).
- From opentennis: nothing new to take — its rollback is 2–4 players and its lobby fixed at four; the
  vendored ggrs has no player cap of its own (checked: `third_party/ggrs` sizes everything by
  `num_players`).

## What the original does

Nothing to port: the original has no online play; this is the port's own (an addition, AGENT.md).

## How it was proved

**The harness** (`cargo test --test netplay -- --include-ignored sixteen`, opt-level 2, this
32-thread machine shared with three other slots' builds and tests, so frame times are upper-ish
bounds). Bots: `mash`'s pads (throttle pressure, stick sweep, trick chords) on every seat; link 5 %
loss, 3 rounds (100 ms) late. 300 rounds:

| seats / machines | frame mean | frame worst | re-runs (300 ticks) | sent a tick, each | state bytes | ggrs save |
|---|---|---|---|---|---|---|
| 8 / 4 | 1.5–2.1 ms | 5.9 ms | 540–873 | 6.3 dgrams, 405 B (12 kB/s) | 36,340 | 3.1 µs |
| 16 / 4 | 2.3–6.1 ms | 19.2 ms | 440–1548 | 6.3 dgrams, 610 B (18 kB/s) | 72,676 | 4.6 µs |
| 16 / 16 | 2.3–9.2 ms | 19.6 ms | 480–2792 | 30 dgrams, 2,140 B (64 kB/s) | 72,676 | 5.4 µs |
| 32 / 32 | 6.9–17.7 ms | 62 ms | 575–1893 | 62 dgrams, 6,420 B (193 kB/s) | ~145,000 (twice 16's; its line was not kept) | — |

The biggest datagram: 282 bytes at 16 seats (ggrs' input message carries a connect status for every
player, so a datagram grows with the seats: 71 B at 16, 104 B at 32), well under the 1,000-byte
lobby path and `DATAGRAM_MAX`. 16/16 agreed on 244 settled ticks of 300, 16/4 on 257, no desync in any.

**The shell** (`target/release/openmx --headless --frames 600 --timings --online … --seats …`, four
processes on 127.0.0.1 UDP, the disc's default track; script in the session's scratchpad,
`sixteen.sh`): 16 seats dealt round four machines: 586 ticks, 585 settled, 0 desyncs, the same
"state before tick 300" hash (0x4c0dacdbcf73e7c5) on all four, sim 4.2 ms a 60 Hz frame, all 16.8 ms
(paced). 32 seats: 584 ticks, 0 desyncs, one hash on all four (0x9ff976972ea21cc0), sim 7.1 ms a frame.

**What really limits it**, then, is not a number in the code:
- **Rollback re-runs × seats.** A wrong guess re-runs every seat for every tick since; at 32 seats on
  a 100 ms link a frame's worst was 62 ms, two ticks of budget (the gate catches up two a call, so it
  holds 30 Hz on average but hitches). 16 stays under a tick even at its worst (19.6 ms).
- **The host's relay.** Lobby rides are a star: every guest-to-guest datagram goes through the host.
  From the per-machine numbers, at 16 machines the host carries 15 guests × 14 others × ~71 B × 30 Hz
  ≈ 450 kB/s each way, plus its own 64 kB/s; at 32, ≈ 31 × 30 × 104 × 30 ≈ 2.9 MB/s. (Computed, not
  measured over iroh: see below.)
- **ggrs' queues** are per player (128 inputs each) and the save is an `Arc<Vec<Sim>>` clone, 5 µs at
  16 — neither matters. The resync/restart state is 4.5 kB a seat (146 kB at 32), ~125 pieces of
  1,180 bytes on the reliable channel: slower to land, not a limit.
- The **lobby board** carries only counts (`taken`/`seats`), unchanged in size; the **seat list** was
  the one screen that broke past six (it ran into the chat) and is now columns.

## Not verified / not 1:1

- **Frame times are from a shared machine** (three other slots building and testing): the harness's
  are wall-clock per machine frame with 16–32 machines in one thread, not each on its own core.
- **The shell runs were headless with idle pads**: `--online` refuses `--play`, so the shells' riders
  had no input and nothing mispredicted (no re-runs there); the re-run cost is the harness's numbers,
  on the flat floor, not the disc's terrain. GPU drawing of 16–32 riders was not timed (headless).
- **The host's relay bandwidth is computed** from the per-pair datagrams, not measured over iroh's
  QUIC (the offline netplay path the tests use is direct peer-to-peer, no star, no QUIC framing).
- **Lobby and drop-in rides draw only this machine's rider** (found in main.rs, not run): the lobby
  path refuses `world.seats != 1`, and the loop draws and radars `world.seats` only, so past-8 drawing
  and the radar are exercised by `--online` rides alone. Filed as N13.
- **A ride opened to online** opens at `Settings::lobby_seats` (the lobby's Seats row's last pick, 4 by
  default); its pause has no Seats row of its own. Filed as N14.
- **The 16- and 32-column seat panel was not looked at** on screen: it is laid out by numbers (eight
  15-px lines a column, the panel's width shared), and a long name is cut to seven characters.
- **Kick/Leave past 8** were not tried; they work on seat lists by peer and have no count in them.

## Next

Nothing for N12. N13 (draw the lobby ride's other riders) is where a 16-rider ride is first seen.
