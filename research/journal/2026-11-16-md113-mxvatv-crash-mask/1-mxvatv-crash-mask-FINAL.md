# 1 — mxvatv crash mask (FINAL)

Task MD113. 2026-11-16.

## What I did

- Found the two words SLUS-21104's biplane crash test compares. The plane's mask (`+0x30c`) is
  category("VEHICLE"). The struck object's `+0x94` is that collidable's category word.
- Mapped them to the port's categories by name. SLUS-21104's rule is the same one as MX Unleashed's,
  which the port already has as `flight_search::VEHICLE` (0x2000). Only the bit differs, because each
  game builds its category table as it asks for names.
- New test in `crates/game-sim/tests/mxvatv_biplane_wreck.rs`:
  `the_mxvatv_crash_mask_is_the_ports_vehicle_category`.
- One comment line in `flight.rs` (`Flight::post`) saying the mask is the same category by name.
  The port's code is unchanged: SLUS-21104's record strikes no object in the port (MD110), so `ignored`
  stays false for such a plane.
- `research/oracle.txt` `# MD113`: the addresses.
- Filed C35 (later): a capture of the plane striking another vehicle.
- Nothing was taken from opentennis (no collision categories there).

## What the original does

- **Categories.** `./kit fn 0x00113d00 -o atv` turns a name into `1 << its place` in a table of 32-byte
  names, adding the name at the end if it is not there. The port has this as
  `handling::update::category`. Bits depend on the order names were first asked for, so they differ
  between games.
- **The mask.** MXVehicle's constructor (`./kit fn 0x004a49c0 -o atv`) sets `+0x30c` to
  category("VEHICLE"). Nothing else writes it on a vehicle. In biplane3's table, VEHICLE is name 2 of 19:
  bit 0x4.
- **The class.** The biplane's init (`./kit fn 0x004c0150 -o atv`) ORs MACHINE | `+0x30c` |
  DYNAMICOBJECT into its frame mesh's `+0x94`. In the image this is 0x43804 (with PHYSICSBODY and
  PHYSICSOBJECT from elsewhere).
- **The crash test** spares a struck object whose class shares a bit with the mask. So it spares
  anything of category VEHICLE: another machine's frame mesh.
- **MX Unleashed**: the table behind `0x0041db50` has 22 names, with VEHICLE at 13 = 0x2000, which is
  what the port's `flight_search::VEHICLE` already is. The port's other categories are the same names
  in MXU's bits:
  - the plane's collidable 0x42058 = SLUS-21104's 0x43804;
  - scenery 0x220 = SLUS-21104's 0x600 (STATICOBJECT | CAMERACOLLISION);
  - the trunks' 4 = SOLIDFOLIAGE (SLUS-21104's 0x40).
  
  Using SLUS-21104's raw mask 0x4 on the port's categories would wrongly spare the trunks.
- **Other vehicles.** Every machine image (md39a's five cars, md80a's biplanes and kiowas) holds two
  VEHICLE collidables: the player's, in the world, and the class's second object, out of it
  (`+0x20` = 0). In biplane3 the second plane sits 30 ft away but cannot be struck.

## How it was proved

- **A save-state RAM image** (md80a's `biplane3.ee.bin`, plus MXU's `biplane.p2m2_SaveState`):
  - the plane's mask equals SLUS-21104's category("VEHICLE"), read from its table;
  - MXU's VEHICLE is the port's `flight_search::VEHICLE`;
  - the plane's mesh class, the scenery class and SOLIDFOLIAGE, read by name in MXU's table, are the
    port's `flight::COLLIDABLE`, `SCENERY` and `TREE`;
  - the only VEHICLE collidable in the world is the plane's own.
- **The original's crash test**, run in the interpreter on the image, with each of the image's
  collidables as the struck object: every class found among the image's 237 collidables plus every
  single bit of the table (29 classes in all). The plane is touching, at 150 mph, level. It wrecks
  exactly when the class, read in MXU's names, misses the port's VEHICLE: 2 classes spared, 27 wreck.

## Not verified / not 1:1

- **No capture strikes another vehicle.** The task asked for one. It cannot be made from any state
  we have: every machine state has only the player's vehicle in its world (the second object is out).
  The rule is proven on the image with the original's own crash test instead. C35 (later) is the
  capture, if the game has a mode with two machines in one world.
- **The port never strikes an object on SLUS-21104's record** (MD110: no props), so its `ignored` is
  always false. This matches the original in every state we have, because there is nothing else of
  category VEHICLE to strike there. A port that adds SLUS-21104's props must give them their categories
  in MXU's bits by name, as this test does.
- **The collidables were found by a scan** for vtables 0x003b0000–0x003d0000 with their second vtable
  at `+0x1c`. The scan is not the world's own list, so a collidable class with another layout could
  be missed. The single-bit cases cover every name anyway.
- **The PHYSICSBODY | PHYSICSOBJECT bits** of the plane's mesh come from a writer I did not find. They
  do not change the result, since neither is VEHICLE.

## Next

None.
