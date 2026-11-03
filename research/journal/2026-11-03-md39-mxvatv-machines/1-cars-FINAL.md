# 1 — cars (FINAL)

Task MD39. 2026-11-03.

## What I did

- Got the five car files out of the disc. `game_sim::archive::{Psr, Res}` over the MX vs. ATV iso's
  MXVSATV.PSR, `Veh\vehicletype_{4,5,6,7,8}.res` (4 buggy, 5 monstertruck, 6 baja, 7 golfcart, 8 sandrail),
  each holding `Veh\<name>.phy`. Copied to `context/mxvatv-ps2/md39/phy/` (git-ignored; in s1).
- Read the PC factory and the RTTI vtables of MXVehicleBuggy, MXVehicleGolfCart and MXVehicleMonsterTruck
  against MXVehicleCar (`context/decomp/pc.vtables.tsv`, slot by slot).
- Found SLUS-21104's four car vtables and diffed them; paired each differing slot with the PC by body.
- Aligned SLUS-21104's car loader line by line against MX Unleashed's (`./kit fn` both with
  `--limit 2000`, string addresses resolved, keys matched in order) to get the defaults and the field map.
- Diffed the MX vs. ATV files against MX Unleashed's car files, key set and values.
- tests/mxvatv_car_phy.rs: the five files through `car::Car::read`.

## What the original does

research/mxvatv-pc.md "The cars (MD39)" has it all. In short: the factory builds the buggy, the baja and the
sandrail with one class. The subclasses override only the loader's tail ([Animation] boxes; the truck's
RearTireSteerScale), drawing, rider setup and the update's head (odometer, collision on/off, rumble). The
truck also adds to PostProcess. The physics listeners are the car's own in every class:
`./kit dis 0x4c70e0 0x4c71d0 --program atv` shows the buggy's thunks jumping to them. So the five
machines differ by their files and by three loader changes against MX Unleashed.

## How it was proved

- The class and slot map: read off the vtables and the bodies, not run.
- The loader and field map: a line-by-line alignment of the two decompiles, not run. There is no car in
  any SLUS-21104 RAM image here (state04, MD37a's six), so the oracle had nothing to run against.
- `./kit check -- -p game-sim --test mxvatv_car_phy --test car_phy -- --nocapture`: 6 passed, none
  skipped. The new test checks that the five files read, that only the truck reads RearTireSteerScale,
  and that the buggy's tires are 1.1 ft (MX Unleashed's are 1.2).

## Not verified / not 1:1

- Nothing is proved against a SLUS-21104 drive. The brief's "a captured drive of each replays bit-exact"
  is not met (MD39a, MD39b).
- The field map is read off the decompile, not checked against a RAM image of a car.
- Which of the car listeners at this−48 and this−60 is the preforce is not checked.
- `Car::read` ignores the [Wrecks] keys, WreckGravity/Damping/Inertia and [Camera], and reads
  MaxWreckTime as MX Unleashed does (MD39b).
- The truck's PostProcess point (+0x14a0) is not read by the step as far as followed. Its readers are
  not searched for.
- The sandrail and the golf cart read as `Kind::Buggy`. That is right for the sandrail (one constructor),
  but the golf cart's own loader reads extra rider boxes that `Car` does not keep.

## Next

The follow-ups carry the rest. MD39a: capture each car in MX vs. ATV free ride in your own PCSX2 copy, with RAM images. Then check the
field map in research/oracle.txt `# MD39` against those images before MD39b ports anything.
