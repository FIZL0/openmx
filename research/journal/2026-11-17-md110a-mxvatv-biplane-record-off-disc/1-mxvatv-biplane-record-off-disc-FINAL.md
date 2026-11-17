# 1 — mxvatv biplane record off disc

Task MD110a. 2026-11-17.

## What I did

- `flight::MxvatvRecord::load(part, rays)` and `::read(phy, psg, col, curve)`: SLUS-21104's plane
  collision record built from the disc, in the way MD149b's `atv_car::Load::record` builds the car's.
  The part reader's words (`handling::mass::Part`, `[Mass Properties]` read with the skin model's box) are
  laid at scratch addresses over the record's constructor words and read back with `record::read` /
  `record::held`. The rays are the skin's `…M.col` ray probe (`flight_search::Rays::of(..).pairs()`).
- `Flight::with_mxvatv_record`, so a caller can put the record on a `Flight::new_mxvatv`.
- The constructor words differ from the car's in two places: `+0x24` heightfield rays 1 (the car's frame
  turns them on only when tipped), and the class word `0x43804` (the car's is `0x43806`).
- Copied off MXVSATV.PSR into `context/mxvatv-ps2/md110a/`: `Veh\Col\Biplane{,2,3,4}{M,F}.col` and
  `Veh\Anim\biplane{,2,3,4}.psg`. PSR's `Veh\biplane.phy` is byte-identical to md80b's copy.
- Filed MD110a1: wire the record into the shell's pack planes (`crates/openmx/src/vehicle.rs` is not on
  this task's file list).
- opentennis: nothing relevant (no collision record there). I took the build from atv_car's MD149b
  `Load::record` and adapted it.

## What the original does

- The plane's record (`*(plane+0xc8)`, the record class of F31b4) is filled by the shared part reader from
  `biplane.phy`'s `[Mass Properties]`. biplane3's words confirm it:
  - mass `CollisionWeight 10000 * 0.0310559` = 310.559;
  - scenery spring `|32.2 * 3 * (1500*0.0310559)|` = 4500, damping 0.7 (the `SpringDamping` default;
    the file's `Damping` key is not read here) = 641;
  - actor spring from the `BodySpringStiffness` default 10 = 15000 / 1170;
  - `MaxPenetration` 0.5, body penetration 0.25;
  - surfaces `FrictionZone1` 0.5 then ones;
  - impulse mask `BodyAngularImpulseScale` ones; facing cos 0 = 1.
- Biplane (model one) runs `BiplaneM.col` (44 rays). biplane3 (model two) runs `Biplane3M.col` (36 rays).

## How it was proved

All in `crates/game-sim/tests/mxvatv_biplane_replay.rs`, against MD80a's save-state RAM images:

- `biplane3s_record_off_the_disc_is_its_images` and `biplanes_record_off_the_disc_is_its_images`:
  - every record word the flight does not move is equal (flags, tuning, capacity, probes, mask);
  - every resolution field is equal (mass, scenery and actor responses, surfaces, impulse mask,
    collidable class, actors / collides-with);
  - the held mass equals the body's `+0x18`;
  - the part's rays are bit-exact (36 and 44);
  - the inverse inertia is within 1e-4 (it is 1.8e-5 and 2.0e-5 off).
- `biplane3_flies_on_the_discs_record` and `biplane_flies_on_the_discs_record`:
  - every stretch is flown as `the_port_flies_*` flies it, but with the disc's record in place of the
    image's (only the state words are carried over);
  - every stretch reaches the same pins as the image's record, word for word. This includes biplane3's
    stretch from 2000, whose hull strikes MC02's terrain at 2060 (three heightfield contacts, resolved).

## Not verified / not 1:1

- The record's inverse inertia (and inertia) is 1.8e-5 (biplane3) and 2.0e-5 (biplane) relative off the
  image's.
  - Cause: the part reader takes the model's box in the world, where the plane stood when it was loaded.
    That spot is not in the image.
  - What I tried: frames at the origin, the plane's frame now, and its three stored spawn-like positions
    (`+0x50`, `+0xbd0`, `+0xd40`). None was bit-exact.
  - Impact: the resolution reads this field only when two records meet ("shared"). A terrain strike never
    reads it, so both flights stay bit-exact. A plane meeting another SLUS-21104 record would see the
    difference, but no other record is in the port's worlds.
- No shell plane uses the record yet. The wiring in `crates/openmx/src/vehicle.rs` is MD110a1 (outside
  this task's files). Not seen in the app.
- The model's box is read with the model stood at the origin facing +z (`quad::spawn_frame`), as MD149b
  does for the cars. This is a choice, not the original's spot (see the first point).
- Skins biplane2 and biplane4 (their `.col`/`.psg` were copied) have no image, so they are not compared.

## Next

MD110a1: put `MxvatvRecord::read` on the pack plane in the shell.
