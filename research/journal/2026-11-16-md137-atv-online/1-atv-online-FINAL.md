# 1 — atv online (FINAL)

Task MD137. 2026-11-16.

## What I did

- Found the work already on main: MD152 (b0ca104b9, journal 2026-11-15-md152-atv-online-ride) gave
  Kind::Atv its Wanted byte (6, vehicle.rs `Wanted::byte`/`of`), put each seat's quad in its Sim as a
  hashed, rolled-back lane (game_sim::quad::Ride), and switches it on the same tick on every machine.
  MD152 is a superset of this task; wip/MD137 (4cadee83a) was an earlier, unmerged try at the same
  thing and is dropped.
- Added the one case MD137 names that MD152's test did not: only one seat switches
  (crates/openmx/tests/online_atv.rs `one_seat_switches_to_its_quad_and_both_agree`). The host stays
  on its bike (`1P off`), the guest rides its quad; both shells print the same `online: quads` line,
  0 desyncs, and the same state hash before tick 300.

## What the original does

Not an original behaviour: the original has no online play. Netplay agreement only.

## How it was proved

Two headless online shells on free09, 420 ticks (tests/online_atv.rs), and MD152's both-seat test
beside it. Seen in one run: `online: quads 1P off, 2P atv_Pro250A 657.9 ft 1 wrecks` and
`0 desyncs, state before tick 300 0x57f9c1ee94187e44` on both machines.

## Not verified / not 1:1

- Everything in MD152's list still holds (quad not on the snapshot wire — MD168; quads out of the
  contact lists — MD169; `--peer`s do not check the pack — MD170; look/clips — MD171; lobby path not
  run end to end). Nothing new was changed in the code here.
- The `vehicle:` line names no seat, so the test cannot tell from it which machine's seat switched;
  it reads the `online: quads` line for that instead.

## Next

- none: MD137 is closed.
