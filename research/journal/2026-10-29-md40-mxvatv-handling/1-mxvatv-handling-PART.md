# 1 — mxvatv handling (PART)

Task MD40. 2026-10-29.

## What I did

Scoped first. The brief asks for MX vs. ATV's whole bike handling to replay bit-exact under a new setting.
Diffing MX Unleashed's race250p against MX vs. ATV's mx_Race250P.phy, and reading SLUS-21104's step,
showed the whole step differs, not two functions. The clutch, preload, steer integrate, wreck test, an input
ramp (`[Joystick]` InputRampAmount/InputRampTime), GravityScale, 16 FrictionZones, the collision keys
moved and renamed, and the airborne control rewritten around GasGyroTorque. There is also no MX vs. ATV ride
capture to replay. So MD40 does the parts that can be finished and proved now, and the rest is filed:

- `game_sim::handling`: the `Handling` setting (Original | MX vs. ATV), and **MX vs. ATV's backflip**
  ported from SLUS-21104 as it reads there (`backflip`). It arms only with R1 alone, its lip refuses while
  the 360 is on, it decays only with no trick button and the stick at or behind centre, it has no
  clamp, it ends under its floor (strictly), it keeps a clip state for the rider's backflip stunt, and it
  wrecks if a flip ends mid-clip. One more quirk: the delay slot makes equal min/max times give
  torque = hold − min time.
- `mxvatv_ini` / `mxvatv_phy`: an `mx_*.phy` with its moved keys put back where `Phy::read` looks.
  The collision springs go from `[Mass Properties]` to `[Collisions]` under their old names, GravityScale
  becomes Gravity, FrictionZone1 becomes TerrainFriction, and `[Camera]`'s shoulders go to `[FirstPersonCamera]`.
- `Phy::handling`, `Ride::mxvatv` (an `Option<Box<Flip>>`, made only on MX vs. ATV, wired in the snapshot
  and hashed only when `Some`). Under MX vs. ATV the ride calls `handling::ride_backflip` instead of
  `air::backflip`. When a trick pack's moves are laid (Tricks = MX vs. ATV), MD16's rows stay as they were.
  A mid-clip end is latched and taken by the rider as the 360's is (`Wreck::Tail`, cut-short flag for Air
  Wrecks Off).
- Shell: `settings.txt` `handling = mxvatv`, `--handling original|mxvatv`, and a Handling row on the Ride
  tab beside Tricks (off without mods). The bike files come from the trick pack's `mx_*.phy` (the tricks'
  own pack when one is laid). `--online` and lobby rides are always the original's handling.
- research/oracle.txt: the backflip's field map and the five calls it makes (atv_bike_start_stunt …).
- Follow-ups: MD62 (airborne control + GasGyroTorque), MD63 (rest of the step), C15 (an MX vs. ATV ride
  capture), N21 (online agreement), S222 (the backflip's rider side and the 0xf reason).

## What the original does

`./kit dis 0x004e4f20 --program atv` (the program is SLUS-21104's boot ELF with GAME_REL.BIN folded, in
context/elf/atv.elf). The function runs per substep on the bike object (a0) with dt (f12). Its state is
bike +0x1194..+0x11c0, the same layout as MX Unleashed's moved by the PC's offsets. The trick-button byte
is +0x14ec, and its low nibble equal to 4 is R1 alone. The torque goes in through the rigid body's
vt +0x78 (`./kit dis 0x00497530 --program atv`). That is a `vadd.xyz`, so the moment's w is kept.
The clip setter is vt +0x53c, start/end stunt are vt +0x4b8/+0x4ac, and the mid-clip wreck is vt +0x368
with a reason picked by 0x004e8370. The node's world rows sit at node +0x80 (x, y, z), so the two frame
reads are row x's y and row y's y.

## How it was proved

- Function oracle, `crates/game-sim/tests/mxvatv_air.rs` `mxvatv_backflip_is_the_originals`: SLUS-21104's
  own code runs in the interpreter off a RAM image of save state 4 (`context/mxvatv-ps2/state04.ee.bin`,
  eeMemory.bin of `SLUS-21104 (95BB1901).04.p2s` in ps2-decomp-kit-slots/pcsx2/s9). It covers 10,000
  generated states with the real vtables and hooks on the start/end stunt, the wreck entry and the steep
  test. The bike, body, node and collision regions are compared word for word, along with the hooked
  calls' arguments, and the original may write nothing else. Every path has ≥ 50 states: off the lip,
  arming, triggered, waiting, in the grace, off the lip in time, ended, ended mid-clip, turning with the
  right side down/up.
- `an_mx_phy_reads_its_moved_keys`: the key remap on a hand-written file.
- Original hashes: the ride on Original runs exactly the code it ran before (the new field is `None`,
  left out of the hash walk). Every pinned ride/replay hash in the gate is unchanged. Only sizes moved:
  `size_of::<Ride>()` 2448 → 2456 (tests/ride.rs), `size_of::<Sim>()` 11952 → 11960 (tests/hash.rs), and
  platforms.rs' riders' resync bytes 13563 → 13567 (the one `None` byte a rider), digest re-pinned to match.
  The tick hashes in the same test were not touched.
- The brief's "notes/tricks.md is missing" holds only for this repo. The file is openmx-mods'
  (~/repos/openmx-mods/notes/tricks.md), and this task did not touch it.

## Not verified / not 1:1

- **No MX vs. ATV ride replays bit-exact.** There is no SLUS-21104 ride capture (C15), and most of its step
  is still MX Unleashed's under MX vs. ATV's numbers (MD62, MD63). The Handling setting is MX vs. ATV's
  bike files and backflip on MX Unleashed's other steps. That is a blend, not MX vs. ATV.
- GasGyroTorque, the input ramp, ResetTime/ResetEdgeOfWorld and FrictionZones 2–16 are read by nothing.
  FrictionZone1 stands in for TerrainFriction: an assumption (all 16 are equal in every file seen), not
  read from code.
- `ride_backflip` answers the rider's stunt start as always accepted and never clears a 40+ stunt id.
  The port's rider has no MX vs. ATV backflip stunt to refuse it with. The mid-clip wreck is always reason
  0xb (`Wreck::Tail`); the original picks 0xf on what 0x004e8370 tests (S222). The stunt's end is
  latched and dropped, not scored.
- The oracle proves `backflip` alone; the ride's inputs to it (`trick_nibble` from `Controls::trick`, the
  node rows from `Out::mat3`, the stick/hold from `Held`) are taken to be the same values MX vs. ATV
  reads, not checked against a capture. Bit 3 of the nibble (a fourth trick button?) is always 0 here.
- Under Tricks = MX vs. ATV, MD16's rotation rows replace the backflip as before, so Handling's backflip
  does not run there.
- Garage swaps and the lobby/online paths read the disc's `.phy`. Only the ride's first build reads the
  pack's `mx_*.phy` for every seat.
- The settings row and `--handling` were exercised by the menu tests compiling and passing. No ride on
  MX vs. ATV was played by hand or by a shell test.

## Next

MD62 first (the airborne control is where GasGyroTorque lives); C15 when PCSX2 time is free.
