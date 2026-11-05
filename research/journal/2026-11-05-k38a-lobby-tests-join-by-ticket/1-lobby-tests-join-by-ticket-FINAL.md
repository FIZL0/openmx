# 1 — lobby tests join by ticket (FINAL)

Task K38a. 2026-11-05.

## What I did

- `crates/openmx/tests/lobby.rs`: K38's ticket swap went from `pump_tricks` into the shared `pump`, so every
  test that seats a guest uses it. A thread-local `TICKET` holds the last host's `Event::Code` that `pump`
  heard on the test's thread (each `#[test]` gets its own thread, and the host and guest are pumped on it).
  Any `Cmd::Join(code, …)` the menu sends goes out through `join()`, which puts the ticket where the code was,
  the same as pasting a ticket. The menu still types the code, `g.lobby.code == code` is still checked, and
  the handshake and ride still run over two real `Net`s.
- The two drop-in tests (`a_drop_in_whose_setup_picked_the_plane_rides_in_flying`,
  `a_peer_who_drops_in_is_drawn_in_their_own_look`) send their join by hand. They now take the ticket from
  `ho.kept()` (`Online` keeps every event it doesn't use itself) and send `join(code, …)`.
- `pump_tricks` is back to naming the pack and then calling `pump`. K38's own ticket handling there is now
  shared.
- `by_room_code()` turns the swap off for the test that calls it. Only
  `a_host_s_menu_and_a_joiner_s_find_each_other_by_room_code_on_the_lan_and_ride` calls it, so it is still
  the one test that proves a LAN room code is found over mDNS.
- Nothing taken from opentennis: this is test plumbing only. No timeout widened. No product code changed.

## What the original does

Not applicable (kit task: test harness only).

## How it was proved

- `./kit check -- -p openmx --test lobby`: all pass.
- Load runs of the built `lobby` test binary, many copies at once (load average 50-62 from other slots on
  top):
  - 8 at once: 8/8 green, every test passed, the mDNS one included. K38 measured 4-7 of 8 failing before this
    change.
  - 16 at once: every test that joins by ticket passed in all 16 copies. The mDNS test failed in 3 of 16
    ("both seated: not in 30 s": the guest never found the listing). That is the race this task left in on
    purpose, filed as K40.

## Not verified / not 1:1

- I did not rerun the before-change baseline at 8 wide on this machine. K38's journal measured it the same
  day (4-7/8 failing), and I relied on that.
- `a_host_s_menu_and_a_joiner_s_find_each_other_by_room_code_on_the_lan_and_ride` still races the shared mDNS
  group at 16 copies (3/16). The task says to keep it on mDNS, so it is not fixed here. Filed as K40
  (normal): make the guest's mDNS lookup itself robust.
- `crates/openmx/tests/netplay.rs` may join by room code over mDNS too. The task names lobby.rs only, so I
  did not look at or change netplay.rs.

## Next

Nothing for K38a. K40 holds the mDNS lookup itself.
