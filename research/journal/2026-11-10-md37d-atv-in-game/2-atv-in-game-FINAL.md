# MD37d, part 2: the quad in the shell

2026-11-12. MD122 and MD123 have landed: `game_sim::quad::Ride::spawn` and `Ride::rig` build the quad
from its `.phy`, its chassis model and its collision objects alone, and `Ride::step` rides it on any
`sim_world::World`. This session wires the quad into the shell, offline and as an addition.

## What is in

- **The pick.** The installed pack's `mod.json` lists eight handling files under `phy`
  (`Veh/atv_{Race,Pro}250{A,P}.phy` and their `atv_s_` twins). `machine_packs::Pack::phy` reads that
  list. `machine_offers` used to give the quad pack one greyed row, "openmx cannot ride a quad yet".
  It now gives one row per file, not greyed, named by `vehicle::atv_label` ("Race 250 P", "Pro 250 A
  Sport"). The Garage's Quad row (`Key::Machine` on the quads' kind) picks Off or one of them, and
  the pick is kept in `Settings::atv` (`atv = atv_Race250P`). It is written only when set, so an
  older settings file reads and writes as it did. A pack whose rows all carry a `why` stays greyed,
  as before.
- **The flag.** `--atv NAME` does the same pick from the command line. The front end hands the
  setting to the ride through it, and an online ride clears it.
- **The garage.** `ride_atv` finds the quad pack in the mods folder and reads the picked `.phy`. It
  then reads the chassis by the file's name: `atv_s_*` gets `atv_s_chasis.psg` and
  `ATV_S_ChasisM.col`, every other file gets `250r_atv_chasis.psg` and `ATV_R_ChasisM.col`. It also
  reads `WreckATVm.col`, then builds the quad once at the origin to check that every file reads.
  The result is `vehicle::AtvRide` in `Garage::atv`, which every local seat gets a copy of. A line
  says what it read, or why there is no quad.
- **The switch.** `vehicle::Kind::Atv` is a detour kind, like the cars. `Garage::switch_car` takes
  the quad where the player stands: on the ground under them, facing their heading flattened, with
  the world's area as its bounds. While the quad is up, `step_among` steps the traffic and then the
  quad on the seat's bike controls, and the bike sim waits. Leaving drops the ridden quad. On the
  menu side, `menu::Machine::Atv` ("Quad") is last in the pause's switch row for a seat whose
  `Menu::atv` is set.
- **The camera** follows it through `chase::quad`. The target is the body's last position, the drawn
  node's rows, the body's velocity, the quad's airborne flag and its speed.
- **The report**, said as it is left and at the ride's end: distance from where it was taken,
  position, speed, on all four / some wheels / in the air / wrecked, and the wreck count.

## Checked

- `crates/openmx/tests/mxvatv_atv.rs`, on free09 headless with hand-written tapes:
  - Off: no `atv:` line, and the tape's `switch atv` is refused ("this ride has no quad").
  - On with `atv_Race250P`: switching at tick 30 with the gas held, the quad had gone 469.9 ft by
    tick 280, on all four wheels and never wrecked. `atv_s_Pro250A` builds and rides as well.
  - Picked but never switched to: the bike's `ride hash` equals the one with no quad picked.
- `crates/openmx/tests/menu.rs` `the_quad_row_picks_a_handling_file_and_the_pause_offers_it`: the
  row's choices, Off by default, stepping the pick, the settings file round-tripping it, and the
  switch row.
- The `machine_offers` unit test now expects the eight handling-file rows.
- With no quad picked, nothing reads the pack and no step changed. The existing replay and hash
  tests in the gate cover that.

## Not verified / not 1:1

- **Not drawn.** The quad has no model in the picture, and neither does its rider: no chassis, no
  `.sub` textures, no posed nodes, and none of the quad's or rider's clips. The brief's "ridden
  with the quad's clips" is not met. Filed as MD135.
- **FC_ATV.ini cameras, engine sound and particles are not in.** The quad uses the bike's chase
  camera (`chase::quad` fills its target block), makes no engine sound and throws no particles.
  Which words MX vs. ATV's quad writes into the camera's target block was not read. Filed as MD136.
- **Offline only.** `Wanted::byte` gives 0 for the quad, and an online ride clears `--atv`. Filed as
  MD137.
- **Controls.** The quad reads the seat's bike `Controls`, as the bike's pad scheme maps them. MX vs.
  ATV's own quad mapping on the pad was not compared.
- **The sport chassis is unproven.** MD122 and MD123 proved the step against the R chassis files
  only. Here the `atv_s_` files are only shown to build and roll forward; no capture is compared.
- **The picks are a menu value, not a tape value.** A recorded tape with `switch atv` replays the
  quad only if `--atv` (or the Garage pick) is given again. The tape does not carry the handling
  file.
- **Where it stands.** The quad is taken where the player stands, not from a parked bay. The quad
  pack lists no bays, and MX Unleashed's machine.txt has none for a quad.
- **No visual check beside the original.** Nothing was compared with MX vs. ATV in PCSX2. The
  distance and state lines are the only evidence that it rides sensibly.
