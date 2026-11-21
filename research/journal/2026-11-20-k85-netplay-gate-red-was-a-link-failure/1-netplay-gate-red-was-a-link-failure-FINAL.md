# 1 — netplay gate red was a link failure (FINAL)

Task K85. 2026-11-20. Concept task: options page https://claude.ai/artifact/EsExUjwopRcbdkoY2t2jgD

## What I did

- Read every saved gate failure in `context/notes/gates/` (251 files, 11-13 to 11-20). `openmx (test "netplay")`
  appears only in BUILD FAILED link errors (11-18 twice, 11-20 in P383's merge2). It never failed as a test there.
  So P383's red is K82's link-failure problem, not this test's.
- Read `crates/openmx/tests/netplay.rs`. The simulated rides (`run_asleep`, `joined_mid_ride`) drive ggrs' clock
  through `third_party/ggrs/src/clock.rs`, which is thread-local, so parallel tests don't share it. They are
  deterministic (HashMap send order fixed in 75bbd8d).
- One test is wall-clock and real network: `a_freeride_opened_on_the_lan_takes_two_peers_mid_ride` (three
  `Net::start` peers, LAN only, real-time frames, 150 s limit). Memory notes it red under load in N11 (10-24) and
  R10 (10-26) with "No open lobby has the code": the guest looks the code up over the machine's one shared mDNS
  group, the race K38/K38a fixed in lobby.rs by joining with the host's ticket. `peer::Peer` already keeps the
  ticket (`Event::Code` → `self.ticket`), so the same fix is a test-only edit.
- `crates/openmx-net/tests/netplay.rs` (also a binary named netplay) joins by code over the LAN in
  `a_host_and_a_joiner_find_each_other_by_room_code_on_the_lan`; that stays as the LAN code proof.
- Options page: A join the LAN test by ticket (recommended), B close with no change (K82 covers the gate),
  C ignore it or put it in `[flaky]`.

## What the original does

Not applicable (test tooling).

## How it was proved

Gate logs read by hand (grep of `context/notes/gates/*.failed` for netplay). No test run: it's a concept task.

## Not verified / not 1:1

- Did not reproduce the October mDNS red on today's code (K42 since made the code lookup sturdier, so it may be
  rarer now): a concept task builds nothing; the build task should run 8–16 copies of the binary at once
  (memory: lan-lobby-tests-race-the-shared-mdns) before and after.
- Gate failure files only go back to 2026-11-13; older netplay test reds are known only from memory notes.
- Option A leaves the test's real-time timers (recover SILENT 3 s) in place: a stall over ~3 s can still change
  its `restarts == (2,2,1)` assertion. Not measured.

## Next

The person picks an option on the page; the build task follows it.
