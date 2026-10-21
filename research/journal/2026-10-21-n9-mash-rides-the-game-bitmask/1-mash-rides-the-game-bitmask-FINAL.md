# 1 — mash rides the game bitmask (FINAL)

Task N9. 2026-10-21.

## What I did

- `crates/openmx/tests/netplay.rs`'s `mash` now builds its pads from `Control::*.mask()` (Cross, Circle + Up on
  the chord, Square on phase 7). It writes Cross's pressure through `Control::Cross.binding().pressure`, and
  only when Cross is held. The pressure goes `40 + 21 * phase`, so it tops out at 250 and no longer wraps a u8.
- New test `the_mashed_pads_are_a_ride` rides `mash` for 900 ticks through `Rides` with the `after`
  hook and counts the `Controls` the sim actually got, per seat. It requires: gas on more than 8/11 of the ticks
  at more than 5 different strengths, brake, stunt and d-pad up each at least once, d-pad down never, and
  chassis speed above 10.
- The brief overstated the bug, and I measured the old behaviour before changing it. The analogue controls
  are read from their pressure bytes only (`CONTROL_MAP`), and the old `mash` wrote Cross's byte 14
  directly, so **the throttle was open**: 820 of 900 ticks, top speed 28.5. The 80 ticks it missed are
  phase 10, where `40 + 23 * 10` wrapped to 14 as a u8, which is under the 0x19 press threshold. The real
  damage: `0x4000` held d-pad **down** on every tick but phase 7's (820 ticks, so the trick decoder saw a
  held d-pad throughout). `0x8000` (meant as Square) was d-pad left, so the bikes never braked. The chord's
  `0x2000 | 0x0010` was right + triangle, so they never held a stunt (Circle). The new test failed on
  exactly that (`brake: 0, stunt: 0, down: 820`) and passes after the fix.
- `crates/openmx-net/tests/netplay.rs` (also on the brief's file list) builds no pads from button bits, so
  it had nothing to fix.
- Nothing taken from opentennis: its pads use tennis's own bitmask, so there was nothing to copy.

## What the original does

Nothing new. The game's bitmask and pressure bytes are `CONTROL_MAP` (read from the game's RAM in an
earlier task), and `Controls::from_pad` is how the ride reads them.

## How it was proved

`the_mashed_pads_are_a_ride` (crates/openmx/tests/netplay.rs), 3 seats × 900 ticks. I ran it on the old
`mash` and it failed, then on the new one and it passed. The rest of the netplay suite (15 tests) still
passes with the new pads.

## Not verified / not 1:1

- The pads are a test fixture, not a recording of a human, so nothing here is compared with the original.
  They are proved only to be what the sim reads as throttle, brake, stunt and steering.
- The `speed > 10` bound is loose on purpose (the old pads reached 28.5 on the flat floor). It shows the
  bikes move. It does not pin a speed, because the netplay tests compare machines with each other, not
  with a reference ride.

- The first `./kit done` gate failed in `tests/lobby.rs`'s room-code-on-the-LAN test ("both seated: not in 30 s")
  with three other check runs going. This task does not touch the lobby, and the test passed 15/15 run alone,
  so I took it as a load flake and did not debug it.

## Next

Nothing for N9.
