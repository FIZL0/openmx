# 1 — trick pack test joins by ticket (FINAL)

Task K38. 2026-11-05.

## What I did

- **What it raced:** the machine's one mDNS group. The guest joins by the 4-letter room code, and on a LAN
  with no code servers it finds the host only from the host's mDNS listing (`resolve` → `board::lan_code`),
  giving up after `LAN_CODE_WAIT` (20 s) with "No open lobby has the code …". Every LAN test in every slot's
  gate multicasts on the same group at once, and under that the guest can miss the host's listing for the
  whole 20 s. Load on the CPU has nothing to do with it: the test uses 0.1 s of CPU time and spends the rest
  waiting for mDNS.
- **Reproduced before the fix:** I ran 16 copies of the test binary at once, filtered to this test. Without
  the fix, 2 of 16 failed with `both seated: not in 30 s` and the guest showing "No open lobby has the code
  3C88/AUPF". The passing copies took 3–19 s, against 1.7 s for one copy alone, and a timing probe put all of
  that time in the "both seated" step (the other steps took 20–140 ms).
- **The fix (test only, `crates/openmx/tests/lobby.rs`):** `pump_tricks` now keeps the host's ticket
  (`Event::Code`, which the host sends once `addressed` has its direct addresses and before the room code).
  When the guest's menu sends `Cmd::Join(code, picks)`, `pump_tricks` swaps in the ticket before the command
  reaches the network, just as a pasted ticket would arrive. `resolve` takes a ticket as it stands, so the
  join needs no lookup and no mDNS. The guest's menu still goes to Join by Code and types the room code on
  the on-screen keyboard. The handshake, the room naming its pack, the ride and the refusal all still run
  over the two real `Net`s. No timeout was widened.
- Nothing taken from opentennis: this is this repo's own test harness.
- Follow-up **K38a** (critical): the other lobby.rs tests that seat a guest by room code over mDNS have the
  same race. `a_host_s_menu_and_a_joiner_s_find_each_other_by_room_code_on_the_lan_and_ride` stays on mDNS,
  as the one test that proves finding a room by its code on the LAN.

## What the original does

Nothing here: this is the online front end and its test, not the original's game logic.

## How it was proved

- 16 copies of the test binary at once, filtered to this test, three rounds: 0 of 48 failed, each in
  0.25–0.48 s (before the fix: 2 of 16 failed, the rest took 3–19 s). Every copy's refusal round printed
  "guest told: … trick pack mxvatv …", so both rounds ran.
- 8 copies of the whole lobby binary at once: this test passed in all 8, while the siblings that still join
  over mDNS failed in 4–7 of the 8 (the follow-up K38a).
- `./kit check --all` through `./kit done`.

## Not verified / not 1:1

- The fixed test no longer covers a guest finding this room by its room code over mDNS. Reason: that lookup
  is the race itself. `a_host_s_menu_and_a_joiner_s_find_each_other_by_room_code_on_the_lan_and_ride`
  covers it, and that test can still fail when many slots' gates run at once (K38a).
- I could not see the failure message from F31b5b's merge gate: the runner keeps no log of a gate that
  passed on its rerun. That the merge gate failed for this reason is inferred from the repro above. Every
  failure in the repro was this one.

## Next

Nothing for K38. K38a does the siblings.
