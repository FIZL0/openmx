# 2 — mxvatv cars in game (FINAL)

Task MD39c. 2026-11-14. This picks up after MD150 (the car built from its files, the MD148 of part 1),
MD149 (the car stepped on MX Unleashed's worlds as `atv_car::Ride`) and MD135 (the port's replays) landed.

## What I did

The cars follow MD37d's quad pattern: a machine pack in openmx-mods, a `vehicle::Kind`, a flag and a
setting that are off by default, and a row in the pause's switch list.

- **openmx-mods** (branch `openmx-MD39c`, commit 55600e5, pushed):
  - `import-atv cars` builds the `mxvatv_cars` machine pack. It copies the disc's `Veh\vehicletype_4..8.res`
    members whole: each car's `.phy`, its chassis `.psg` and its collision objects, plus its textures and
    animations.
  - `mod.json` has `machine: mxvatv_car` and a new `cars` list, `{car, phy, psg, col}` per car. It has no
    builds or skins.
  - `import-atv cars check DIR` reads each `.phy` and `.psg` back through game-sim's readers.
  - modcheck now accepts a `cars` list in place of builds or skins, and checks that each listed file is in
    the pack.
  - `build.sh --disc --out` builds the pack too.
  - The `.phy` and `.psg` files are byte-identical to `context/mxvatv-ps2/md39/{phy,model}`, the files
    MD150 and MD149 were proved on.
- **openmx `machine_packs.rs`**: `Pack::cars` (a `CarFiles` list) is read from `mod.json`. A pack that
  has only `cars` is a pack.
- **openmx `vehicle.rs`**:
  - `Kind::MxvatvCar` (word `mxvatv_car`) and `Garage::mxvatv_car`. `Garage::new` refuses it as a start
    vehicle; a player can only switch to it.
  - `MxvatvCarRide` builds `atv_car::Load` once from the pack's `.phy` and `.psg`, as MD149's test does.
  - Each switch to the car makes a fresh `atv_car::Ride` on the ground under the player, facing the
    player's flattened heading, with its lowest tire a foot up (MD149's `over`). Leaving the car drops it.
  - `step_among` steps the car on seat 0's pad through the ride's bindings, with traffic stepped as it is
    for a car.
  - `report()` says the car's distance from where it was taken, its speed, its state and its wreck count.
    It also gives the body's position and velocity bits and the taken spot's bits.
- **openmx `main.rs`**:
  - `--mxvatv-car NAME` and settings.txt's `mxvatv_car`. Both are offline only, and both are blanked in
    an online room, as `atv` is.
  - `ride_mxvatv_car()` reads the pack. Every seat's garage gets a copy.
  - The car's line is printed when the player leaves it and at the ride's end.
  - The car pack is left out of the Garage's machine offers rather than greyed as "cannot ride".
- **openmx menus**:
  - `menu::Machine::MxvatvCar` ("MX vs. ATV Car") is the switch row's last entry, offered to a seat whose
    garage has the car.
  - `Settings::mxvatv_car` is written only when it is set.
  - The cameras' screens and the pause's modifiers page treat the car as the bike.
- **openmx `chase.rs`**: `mxvatv_car(ride)` is the car's camera target block. It is filled the way the
  quad's is: body centre of mass, node rows, "no tire down" and the flat speed.
- Nothing was taken from opentennis. Nothing in `crates/game-sim` changed.
- Follow-ups filed:
  - MD157: draw the cars and give them their own camera and sound.
  - MD158: a Garage row for the cars.
  - MD159: MD39b's drives through the shell's car. It is blocked on MD149a.

## What the original does

Unchanged from MD39, MD39b and MD149: all five cars run MXVehicleCar's step, and the original has no car
on an MX Unleashed track. Addresses are in `research/oracle.txt` under `# MD39`, `# MD39b` and `# MD106*`.

## How it was proved

`crates/openmx/tests/mxvatv_cars.rs`, run with the disc and the pack installed in this slot's `mods/`
(both tests passed in 17 s):

- **Off by default** (`off_by_default_there_is_no_mxvatv_car`):
  - A tape that switches to `mxvatv_car` at tick 30 is refused ("this ride has no MX vs. ATV car").
  - No pack is read and there is no `mxvatv car:` line.
  - MX Unleashed's own buggy still switches in as `buggy`.
- **On** (`each_mxvatv_car_drives_in_the_shell_on_the_step_it_is_proved_on`):
  - For each of the five, the shell switches in at tick 30 on Cross held and leaves at 280. The car went
    more than 100 ft on the gas.
  - The test builds the same car off the same pack files at the spot the shell reported, over the same
    split free09 as MD149's test. It feeds the same pad report and steps exactly 250 times.
  - All five reach the shell's six body position and velocity words bit for bit after 250 steps, one per
    tick from switch-in to switch-out. So the shell runs the same `atv_car::Ride` that MD149 proved rides
    and that MD135's replays prove the step on, with nothing added or lost on the way.
- **Picked but never switched to**: the bike's ride hash equals the hash with no car picked.
- In the by-hand run, the buggy went 560 ft at 116 ft/s and the golf cart 424 ft at 62 ft/s. Neither
  wrecked.
- `tests/menu.rs`:
  - `an_mxvatv_car_is_off_by_default_and_the_pause_offers_a_picked_one` covers: off by default, settings
    round-trip, the switch row offering the car after the quad, the `Kind` word, and refusing it as a
    start vehicle.
  - The settings round-trip test now sets `mxvatv_car` too.
- `tests/menu.rs`, `machine_packs.rs`, `mxvatv_atv.rs` and `mxvatv_cars.rs` pass. `./kit audit` is OK.
  openmx-mods' `the_car_pack_is_the_discs_and_openmx_reads_it` passes with `DISCX_ISO`.

## Not verified / not 1:1

- **"Each drives its MD39b replay" is not met literally.** MD39b's drives run through `atv_car::Ride` only
  after MD149a, which is still open. Instead, the shell's car is held bit for bit to `atv_car::Ride` on
  free09, the type MD149 rides headless. The literal replay through the shell is MD159.
- **The cars are invisible, silent and use the bike's chase camera**, with no particles and no
  per-car camera presets. They are physics only, as the quad was in MD37d. MD157 covers this.
- **No Garage row.** Only `--mxvatv-car` and settings.txt's `mxvatv_car` pick a car. A player then
  switches to it from the pause. MD158 covers the row.
- **The spawn is the shell's own**, so it is not 1:1:
  - The height (lowest tire a foot over the ground) and yaw (host `atan2` and `sin`/`cos`) are worked out
    in host floats.
  - The drag is 0.
  - The car is made fresh on every switch, with no parked car.
  - The original never spawns a car on an MX Unleashed world, so there is nothing to compare against.
- **Offline only.** The cars are blanked in an online room, and `Wanted::byte` sends 0 for them.
- **Only seat 0's pad drives the car** (`sim.input.pad(0)`), as for MX Unleashed's cars in `step_among`.
  A second seat has its garage copy, but has not been ridden in a car.
- **MD149's own gaps carry over**: the calls `atv_car::Ride` drops on purpose (sounds, rumble, spatial
  sync, network, the put-back's place, and so on).
- **The pause modifiers page** shows the bike's knobs while the player is in a car. None of them act on
  the car.
- **Not looked at beside MX vs. ATV in PCSX2**: there is no picture to look at yet (MD157).

## Next

Done, once the gate passes. MD157, MD158 and MD159 (after MD149a) carry the rest.
