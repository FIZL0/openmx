# 1 — stadium class (FINAL)

Task MD72. 2026-11-16.

## What I did

- Found MX vs. ATV's reason-15 class. It is the category "STADIUMCOLLISION" (bit 5, `0x20`).
- Found which objects get it: the level's collision objects whose name holds `sxcl.col`, case not
  counted. That is `Data\Col\SxCl.col` (common.pak), placed as `[ObjectN] COL=sxcl.col` by the
  supercross tracks SX01–SX20, freestyle FS01–FS08, motocross MC06–MC08 and SM03 (PC data, grepped).
- MX Unleashed's stadiums use `wall_col.col` (sx01 scene, 640 placements). No MXU prop has the class.
- `world::stadium_class(file)` and `world::Prop::stadium` (set by `Prop::place` from the placement's
  file; false for moving cars and plants). Imported MX vs. ATV maps keep the scene's `COL=` names
  (openmx-mods' import-mxvatv), so their clamps carry it.
- `rider_contact::scenery_heard` now also answers whether the body-ray record's struck object
  (the last thing the world rays hit) is a stadium prop.
- `Ride::struck_stadium` (off the wire, set each tick by the rider): the search touched something and
  the struck prop is stadium-class. It feeds both reads the brief named:
  - `WreckIn::struck_class` in rider.rs (was `false`);
  - the backflip's mid-clip steep in ride.rs (was always `0xb`).
- Filed MD72a: the ATV-on-MXU quad (MD123a) still gives every prop the class.

## What the original does

- `./kit fn 0x0052cd00 -p atv`: the mask word = the category table's bit for the name at
  `0x0052c5cc` ("STADIUMCOLLISION").
- `./kit fn 0x00517600 -p atv`: for each object in the level list (`+0x3d4` → `+0x3ec` count,
  `+0x3f0` entries, entry `+0x28` object) whose name (`+0x9b`) holds "sxcl.col" (`0x0026cc80`,
  a case-blind strstr), OR the mask into the object's categories (`+0x94`). Called at level start
  from `0x0051d7d0`.
- `./kit fn 0x004e8370 -p atv`: the steep test masks record `+0x20c`'s `+0x94` with it (as MD63c).
- In state04 (nat05) the static props' class words are `0x600`; none has `0x20`. Every saved image is
  nat05 or MC02, which have no clamp.

## How it was proved

- Oracle: `tests/mxvatv_air.rs` `mxvatv_stadium_class_is_the_originals`. The original's tagging
  function runs on state04 over 10,000 generated levels (up to 32 objects each, null objects, names
  in every case, cut short, one letter off, overlapping false starts, non-ASCII letters). Each
  object's class word matches `stadium_class`. A case-sensitive mutant fails it.
- Wiring: `tests/mxvatv_ride.rs` `an_airborne_mxvatv_rider_wrecks_struck_on_a_stadium_wall_only`
  (disc bike rays). An airborne MX vs. ATV rider whose body rays strike a `SxCl.col` wall wrecks at
  15; under `wall_col.col`, or on the original's handling, at 8. Feeding `false` again fails it.

## Not verified / not 1:1

- No live capture of a reason-15 wreck. No saved image or capture is on a track with a clamp. Making
  one needs MX vs. ATV driven by tape to a supercross track: too much for a low task.
- The struck object is fresh each tick. The original's record never empties `+0x20c`. So a tick where
  only the ground rays touch keeps the last tick's struck object in the original. The port says no
  there. Keeping it would add a field to the netplay wire.
- With other riders (a bump), the port searches props and riders apart. The original's single search
  might end on a rider's bike after a clamp, or the other way round. "Last struck" may differ there.
- The backflip's mid-clip steep read is wired the same way, but no test lands a flip on a clamp.
- The other three users of the mask add it to some objects' ignore lists (`0x004855d0`, `0x0048af30`,
  `0x0048cd00`, and likely `0x0048d6a0`). Not followed: what those objects are, and whether the port
  has them.
- The name match folds ASCII case only. The oracle covered non-ASCII bytes, but only the ones it
  generated (é, É, ſ, ˆ).

## Next

- MD72a: give the quad's struck category per prop instead of to every prop.
