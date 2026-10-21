# 1 — online at 30hz (FINAL)

Task N6. 2026-10-21.

## What I did

- **The tick's length is the tick's own, by number.** `net::rides::steady_dt()` (the engine's average on a
  60 Hz clock, ~1/60) is gone. In its place is `rides::tick_dt(tick)`: the `dt` the engine hands the
  simulation on P52's offline schedule (`pace::after_load`, then `pace::tick_clock(t)`). That is 0.033
  (0.032999992 in the port's float) on the first tick after the load, then a ramp up to the 30 Hz cap
  `DeltaTime::CAP_30` within 40 ticks, and the cap for good after that. It stays there because any thirty
  frames of the PS2 clock are 576,576 Timer 0 counts, exactly 1001 ms, so their mean is over the cap. The
  first 64 ticks are worked out once through `game_sim::tick::Engine`; later ticks reuse the last of them.
  The clock is duplicated in `rides.rs` (the tests mount `net/` on its own and cannot reach `pace.rs`),
  and `pace.rs`'s test `the_online_ticks_are_the_offline_ones` holds the two together bit for bit.
- **`Game::step` takes the tick number** (`step(&mut self, tick, pads)`), so a rollback's re-run, a
  resync's `rerun` and a desync report's `replay` all hand a tick the same `dt` as its first run did.
  `Rides` no longer has a `dt` field. `Online::dt()` is now the `dt` of the last tick run, which the
  shell's camera and pose step with.
- **The gate runs on pace's schedule.** In `main.rs` the online ride's `Pacer` uses `pace::TICK`
  (1001/30000 s) instead of 1/60 s. `rollback::gate` still counts each call as one tick of real time, and
  each call now stands for 1/30 s. The browser build follows: `web/worker.js` and `tests/web/peer.mjs`
  call `frame` every 1001/30 ms, and `tests/netplay_browser.rs` uses the same step.
- **Session numbers, re-based to wall time.** `DELAY` 2 → 1 tick and `WINDOW` 80 → 40 ticks keep
  opentennis's 33 ms of input delay and 1.3 s of prediction. `SPREAD` stays 20 calls (time sync's
  correction stays the same 5 % of real time). `HASH_EVERY` stays 30 ticks, which is now one second.
  `Online::summary`'s ten-second hash mark is 300 ticks.
- **Protocol bump.** ALPN `openmx/lobby/1` → `openmx/lobby/2`, so a 60 Hz build cannot connect at all.
  The dev build id is now `"<version> dev, 30 Hz"`, so Find greys out (and Quickplay skips) lobbies of
  older dev builds rather than listing one this build cannot connect to. There is no version byte in the
  plain-UDP `--peer` path (N2's datagrams). Two mismatched `--peer` builds would desync, be reported and
  resync to the host's state, but nothing refuses them up front (listed below).
- **Tests re-based.** In `tests/netplay.rs` a harness round is one 30 Hz tick (`ROUND`, which drives
  ggrs' clock and the recovery's dt). The tests that measure time (bad network, silent/away peer, silent
  host) have their round counts halved, so each covers the same seconds as before: the outage is still
  3 s, the waits still 1 s, the away peer still gone 4 s. The bad-network lag is 3 rounds (100 ms; it was
  5 rounds, 83 ms). The tests that count ticks (desync at 330, codec) are unchanged. `tests/lobby.rs`'s
  lobby ride runs 120 calls 33 ms apart.
- **Desync test.** `a_desync_on_the_ride_is_caught_saved_and_resynced` assumed both machines catch the
  desync at tick 330. After the change the host caught it at 360. I added temporary prints to ggrs and
  found the cause: the host's 330 checksum report was one of the 5 % of datagrams the lossy link drops,
  and ggrs never resends a checksum report, so the host catches it at the next one. The guest did catch
  330. The test now asks for each machine's first desync to be a checksum tick from 330 to 390, and for
  someone to have caught 330. The report's replay is compared through the tick it was caught at. Past
  that, the report's newest ticks may hold guessed pads that the host later re-ran on the real ones (the
  old test compared every hash and passed only by luck). The ggrs prints were removed: `third_party/` has
  no diff.
- Took from opentennis: nothing new. Its numbers (B197/B198) are kept in seconds rather than in ticks.

## What the original does

The original has no online play. What it fixes for this task is the frame: it holds 30 fps and caps
`dt` at 0.03333 after a thirty-frame average (`game_sim::tick::DeltaTime`, P52's notes). Online now rides
that frame and that `dt` sequence.

## How it was proved

- `pace.rs` `the_online_ticks_are_the_offline_ones`: `rides::tick_dt(t)` equals the offline engine's
  `dt` bit for bit over 100,000 ticks (about 55 minutes of riding).
- `tests/netplay.rs` `the_online_step_is_the_30_hz_one`: 0.033 first, the cap within 40 ticks, the cap
  through tick 100,000.
- The whole netplay suite at 30 Hz rounds: bad networks with 2, 4 and one-way peers, desync plus resync on
  the ride and on Tally, silent/away peers, the codec. Also the lobby's ride over iroh
  (`tests/lobby.rs`) and the desktop-against-browser rides (`tests/netplay_browser.rs`, chromium and
  node). `./kit check`: TESTS PASSED.
- The real shell, by hand: two `openmx --headless --ride --frames 600 --online …` processes over
  loopback UDP. 600 pacer ticks took about 20 s (30 a second) while the picture drew 60 fps. Both ended on
  592 ticks with 0 desyncs, and both gave the same `state before tick 300 0x8e3e7d52aafc2800`.

## Not verified / not 1:1

- **An online one-seat ride is not proved equal to the offline ride on the same pads.** Only the `dt`
  sequence is proved equal. Online steps `Sim::poll` + `Sim::step` per seat (`Rides`). Offline goes
  through `Engine::step` → the shell's `World::simulate` in `main.rs`, which is not reachable from a test
  and also carries the garage, swaps, tapes and the engine's RNG hand-off. Both paths existed before N6;
  whether they agree tick for tick was never tested, and still is not.
- **The plain-UDP `--peer` path has no version check.** A 60 Hz build and a 30 Hz one would start a
  session and desync rather than be refused. Only the lobby (ALPN, build id) refuses them.
- **Desync reports written by a 60 Hz build** replay now on 30 Hz `dt`s, so they no longer reproduce the
  ride that made them. Nothing marks a report's tick length.
- **DELAY 1 / WINDOW 40 are chosen, not measured.** They keep opentennis's wall-time numbers, but nobody
  has played a real internet ride at 30 Hz to feel the latency or count rollbacks.
- **ggrs drops a lost checksum report for good** (found here, not changed): a desync is then caught a
  checksum later, one second at 30 Hz. That is the vendored ggrs' own behaviour, and it was there at 60 Hz
  too.

## Next

Nothing for N6. Filed: N7 (a version check on the `--peer` UDP path), N8 (prove an online one-seat ride equals
the offline one on the same pads).
