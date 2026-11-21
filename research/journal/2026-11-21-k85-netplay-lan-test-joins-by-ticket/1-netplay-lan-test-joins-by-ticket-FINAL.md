# 1 — netplay LAN test joins by ticket (FINAL)

Task K85. 2026-11-21. Build of option A from the concept page
(https://claude.ai/artifact/EsExUjwopRcbdkoY2t2jgD, journal 2026-11-20-k85-…); the person picked A.

## What I did

- `crates/openmx/tests/netplay.rs`, `a_freeride_opened_on_the_lan_takes_two_peers_mid_ride`: both guests now
  send `Cmd::Join` with the host's ticket (`peer::Peer::ticket`, kept from `Event::Code`) instead of the room
  code. The ticket carries the host's addresses, so the join needs no lookup on the machine's one shared mDNS
  group, the race K38/K38a hit in lobby.rs. Same pattern as lobby.rs's `join`.
- Asserts the ticket is out before the first join, as lobby.rs does.
- The room code lookup over the LAN stays proved by openmx-net's own
  `a_host_and_a_joiner_find_each_other_by_room_code_on_the_lan`.
- Nothing taken from ../opentennis: it has no LAN lobby test.

## What the original does

Not applicable (test tooling).

## How it was proved

- `./kit check --package=openmx --test=netplay -- a_freeride_opened_on_the_lan_takes_two_peers_mid_ride`: passes.
- 8 copies of the test binary at once: 8/8 pass, each done in 14.0 s.
- 16 copies at once: 16/16 pass, each done in 14.0 s.
- `./kit check --all` through `./kit done`.

## Not verified / not 1:1

- No "before" stress run on the old room-code join: it needs the old binary built beside this one, and the
  October reds (N11, R10) are already on record; K42 has since made the code lookup sturdier, so the old test
  may not have reproduced today either.
- The test's real-time timers stay (recover SILENT 3 s): a stall over ~3 s could still change its
  `restarts == (2,2,1)` assertion. Not seen in 24 parallel runs; not measured under a heavier load.
- P383's gate red itself was a link failure (K82's), not this test; this change does not touch that.
