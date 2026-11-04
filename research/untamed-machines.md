# MX vs. ATV Untamed's machines against MX vs. ATV Unleashed's (MD81)

User, 2026-11-03: "see what machines we are missing and from untamed if there are too and have them work
as mods". Read 2026-11-03 from the two USA discs in `~/repos/openmx-mods/iso/`: Untamed (SLUS-21701,
`MX08PS2.PSR`) and Unleashed (SLUS-21104, `MXVSATV.PSR`). The archives were unpacked, not string-scanned.

**In short:** Untamed adds no new machine. It has the same machine classes, the same `.phy` names and
the same vehicle collision shapes as MX vs. ATV Unleashed. It **drops** the biplane, the helicopter, the
buggy and the stock ATV, and **retunes** six of the machines it keeps: the MX bikes (P files only), the
race ATV (P files only), and the monster truck, baja truck, golf cart and sandrail. Each retuned machine
has its own follow-up task, MD81a–MD81f (below).

## How it was read

- `discx` (openmx-mods, `tools/discx`) built with a scratch `CARGO_TARGET_DIR`: `discx ls untamed '**'`,
  `discx ls untamed 'res\veh\VehicleType_<n>.res:**'`, `discx x …` of every vehicle archive, the loose
  `Veh\*.phy.bpb`, `Veh\*.dat.bpb`, `Rider\Anim\*.phy.bpb` and `res\VehicleAud.res`. On Unleashed:
  `Veh\*.phy`, `Veh\Col\*`, `Veh\vehicletype_<n>.res`, `Aud\carengine.aud`.
- Untamed's `.bpb` files were decoded to INI with openmx-mods' `bpb` (byte-exact round trip, M10).
- `research/md81_phy_diff.py` compares each pair key by key. Values are compared as f32 lists, so
  `0.05` and `0.05000000074505806` count as equal, and strings are compared without case or spaces.
- The boot ELFs and `GAME_REL.BIN` (bsdtar off the images) were checked for the `.phy` and `.res` name
  formats and the `MXVehicle*.cpp` source names. No code was compared (see *Not checked*).

## The vehicle archives

Both games load `VehicleType_<n>.res` by machine kind (`%sVehicleType_%d.res`). Untamed's are in
`res\veh\`. Unleashed's are in `Veh\`, with lowercase names.

| n | machine | Unleashed | Untamed |
|---|---|---|---|
| 0 | MX bike (`MX_S_*`, `MXF_S_50`) | 683,229 B | 379,943 B: the same 16 `mx_*.phy`, cols, `.aud`; sub-objects in `.sub.bpb` |
| 1 | ATV | 669,846 B: `ATV_R` + `ATV_S` (stock) | 238,873 B: `ATV_R` only, the 4 `atv_*.phy` without `atv_s_` |
| 2 | biplane | 446,809 B | **absent** |
| 3 | helicopter | no archive | no archive |
| 4 | buggy | 180,942 B | **absent** |
| 5 | monster truck | 795,266 B | 785,348 B |
| 6 | baja truck | 376,475 B | 364,181 B |
| 7 | golf cart | 343,049 B | 319,894 B |
| 8 | sandrail | 169,829 B | 178,009 B |

Also on Untamed: `res\veh\Vehicle_mat.res`, `res\veh\Vehicle_tex.res` (36.5 MB), `res\veh\AIParams.res`
(`rider\AIChrctr.aip`), and `res\VehicleAud.res` (`aud\carengine.aud`, `aud\machine.aud`).
`res\UI\Vehicle\VehUI_*.res` holds the 696 garage models: `MX_S_{125,250,250F,450F}_{Honda,Suzuki}` (66
each), `MX_S_500_Honda` and `MXF_S_50_Honda` (4 each), `ATV_R_{250,450F}_{Kawasaki,KTM}` (33 each),
`BajaTruck`, `MonsterTruck` and `SandRail` (8 each), and `GolfCart` (4). There is no buggy, biplane or
helicopter model.

The machines that are gone still leave traces. `Veh\buggy.phy.bpb`, `Veh\kiowa.phy.bpb` and
`Veh\atv_s_*.phy.bpb` are loose on the PSR, as are the `biplane*`/`kiowa*`/`buggy*` `.psg` and `.col`.
`Veh\Vehicles.dat` keeps a `[Buggy]` entry, and the ELF keeps `%sBuggy.phy`, `ATV_S_`, `%s%sATV_S.mcf`
and `STR_VEHICLE_BIPLANE`/`_HELICOPTER`. But **`GAME_REL.BIN` has no `MXVehicleBiplane.cpp` or
`MXVehicleHelicopter.cpp`**, while Unleashed's has both. The classes Untamed keeps are MXVehicle,
MXVehicleMotocross, MXVehicleMotoShared, MXVehicleAtv, MXVehicleCar, MXVehicleBuggy, MXVehicleGolfCart,
MXVehicleMonsterTruck and MXVehicleAI. The menus never offer the buggy (it has no archive and no VehUI
model), and nothing names `biplane.phy`.

## Machine names → `.phy`

Untamed builds the file name exactly as Unleashed does: the same strings in the same order in the boot
ELF (`atv_`/`atv_s_` + `%s%sRace50P.phy` … `%s%sPro500P.phy`; the `ATV_R_`, `MX_S_` and `MXF_S_` prefixes;
`_450F_`, `_250F_`, `_125`, `_250`, `_500`; `%sMonsterTruck.phy`, `%sBaja.phy`, `%sGolfCart.phy`,
`%sSandRail.phy`). The machine classes are also Unleashed's: MX 50/125/250/250F/450F/500 and ATV_R
250/450F. Only the liveries are new: Untamed's fictional brands (`Storm` = the `_Honda` subs, `Swift` =
`_Suzuki`, `Raven` = the Kawasaki quads, `Striker` = the KTM ones) replace Unleashed's licensed skins.
So no new `.phy` is needed for a class. Which `.phy` a 250F or 450F loads is the same question in both
games, and MD37/MD40 answer it for Unleashed.

## `.phy` by `.phy` (Unleashed → Untamed)

All 35 of Untamed's loose `.phy.bpb` decode, and each of the 24 copies in the `VehicleType` archives is
byte-identical to its loose copy. Unleashed's `biplane.phy` has no Untamed counterpart.

| file | sections / keys | differences |
|---|---|---|
| `mx_{Race,Pro}{50,125,250,500}A` (8) | 17 / 999 | none |
| `mx_{Race,Pro}{50,125,250,500}P` (8) | 17 / 999 | 5 each, all `[Wrecks]` (below) |
| `atv_Race250A`, `atv_Pro250A` | 17 / 999 | none |
| `atv_Pro250P` | 17 / 999 | 19, all `[Front Tire]` |
| `atv_Race250P` | 17 / 999 | 30: `[Front Tire]` (25) and `[Wrecks]` (5) |
| `atv_s_*` (4) | 17 / 999 | none (not loaded: no `ATV_S` archive) |
| `monstertruck` | 77 / 1222 | 6 (and the section below) |
| `baja` | 77 / 1208 | 51 (and the section below) |
| `golfcart` | 77 / 1213 | 4 (and the section below) |
| `sandrail` | 77 / 1206 | 14 (and the section below) |
| `buggy` | 77 / 1205 | 0 (and the section below); not loaded |
| `kiowa` | 8 / 110 | none; not loaded (no helicopter class) |
| `WreckBike`, `WreckBike50`, `WreckBlade` | 1 / 32, 32, 24 | none |
| `Rider\Anim\mrider`, `mrider50` | 49 / 585 | none |

**Every car's `[FrontRightTire Surface 10]`.** Unleashed's text has a stray `s` before the header
(`s[FrontRightTire Surface 10]`, e.g. line 786 of `baja.phy`). Untamed's compiled file has the section
under that name. Whether SLUS-21104's text reader takes the `s…` line as that header, or merges those
keys into Surface 9, is not known. It matters to MD39b as much as to Untamed.

**MX P bikes, `[Wrecks]`** (all eight; RacingDown/FreestyleDown were 48 on Race125P, Race250P and
Race500P, and 45 on the other five):

| key | Unleashed | Untamed |
|---|---|---|
| FrontWreckSpeedLimit | 12.5 | 14 |
| BackWreckSpeedLimit | 15 | 17 |
| SideWreckSpeedLimit | 13 | 14.5 |
| RacingDownWreckSpeedLimit | 45 / 48 | 54 |
| FreestyleDownWreckSpeedLimit | 45 / 48 | 54 |

**ATV P, `[Front Tire]`.** Both files change: TireRadius 0.75 → 0.9, MaxSteerAngle 30 → 35, and the
side-force curve 2 and curve 5 coefficients (`SideForce*`, `SlippingSideForce*`, `BrakingSideForce*`,
`MaxSideForce2`, `SideForceDropoff2`). `atv_Race250P` also changes BodyTireForcePosition z 0.75 → 0.5,
MaxBrakingTorque2 325 → 150, MaxSideForce5 5 → 4.85 and SkidSideForce5 4.85 → 4.75, and drops
`WheelDrag5 = 0`. Its `[Wrecks]` limits rise by about 11%: Front 12.5 → 13.8, Back 15 → 16.7, Side
10 → 11.1, both Down limits 80 → 88.8. The full list is `md81_phy_diff.py`'s output.

**Monster truck:** Weight 2600 → 3500, GravityScale 1.2 → 1.65, FrictionZone1 0.5 → 1, and `[Preload]`
switched off (PreloadTime 0.25, MaxPreload 2500, MaxPreloadTime 60 → 0, 0, 0).

**Baja truck:** Weight 2500 → 3500; `[Engine]` HP2 90 → 100, ShiftUpDuration 0.3 → 0.175,
ShiftUpThrottle 0.5 → 0.2, ShiftDownDuration 0.25 → 0.3, ShiftDownThrottle 0.35 → 0.6. Front tires
MaxCompression 2.5 → 1.5 and Spring 0.26 → 0.5; rear tires MaxCompression 2.8 → 2.3, and FrictionPeak
2.70 → 2.85 on all 16 surfaces of both rear tires. `[Joystick]` GasRateLimit 4 → 3, MaxAnalogThrottle
0.8 → 0.1, DigitalThrottleSpeed 50 → 25, DigitalSideSlipAngle 15 → 12. `[Preload]` off.

**Golf cart:** `[Joystick]` MaxAnalogThrottle 0.8 → 0.1, and `[Preload]` off (MaxPreload was 1600).

**Sandrail:** Weight 2000 → 2300, CofG (0, 0.75, 0.5) → (0, 1, 1), the `CofGTravelBack = 0.0` key dropped,
front Spring 0.6 → 0.5; rear MaxDecompressionRate 8 → 7.5, Spring 0.3 → 0.35 and ReboundDampingRatio
0.3 → 0.5. `[Preload]` off.

## Beside the `.phy`

- **Vehicle `.col`.** All 38 in Untamed's `VehicleType` archives are byte-identical to Unleashed's
  `Veh\Col\` files of the same name (bike, ATV_R chassis, WreckBike/WreckATV, every car's F/M and skins
  2–5).
- **Particles.** The `.emt` emitters are all identical. Compared with the Unleashed copy in the same
  archive, `mxparticles.dat` and `atvparticles.dat` add `DiscardWhenSoulTreeMissing = TRUE` to
  `[Right Exhuast]`. The ATV's two particle sheets use `mx_SM_ttrack` for the rear tire tracks instead
  of `mx_ttrack`. `mx`/`atvparticlezones.dat` name `none` for zones 2, 7 and 8 in most effects. The car
  sheets (`monster`, `baja`, `golf`, `SR`) are unchanged.
- **Sound.** `machine.aud`, `event.aud` and the five bike `vehicle*.aud` are unchanged. In
  `carengine.aud`, 86 key volumes are roughly halved (e.g. `[BajaLoop1]` Key1Vol 95 → 44).
- **The rider** (`mrider.phy`, `mrider50.phy`): unchanged.

## What this asks of the port

The game has no new machine kind, so there is no new class to port, only the six retunes. The step code
may still differ, because Untamed is a later build of the same engine. No code was compared, so each task
first checks SLUS-21701's step against SLUS-21104's in PCSX2 before it ports. Each is an addition, off
by default, shipped as a mod pack in openmx-mods. They build on Unleashed's machine work: MD40's moto
step for the bikes, MD37b/MD37d for the ATV, MD39b/MD39c for the cars. Untamed save states are in
PCSX2 copy 9 (`~/repos/ps2-decomp-kit-slots/pcsx2/s9/PCSX2/savestates/SLUS-21701 (08FE7DFD).0{5,8,9}.p2s`;
state 5 is Custom Event > Free Ride's track list, per openmx-mods `notes/accuracy.md`).

| task | machine | after |
|---|---|---|
| MD81a | Untamed's MX bikes (the P files' wreck limits) | MD40 |
| MD81b | Untamed's race ATV (`atv_Race250P`, `atv_Pro250P`) | MD37b, MD37d |
| MD81c | Untamed's monster truck | MD39b, MD39c |
| MD81d | Untamed's baja truck | MD39b, MD39c |
| MD81e | Untamed's golf cart | MD39b, MD39c |
| MD81f | Untamed's sandrail | MD39b, MD39c |

No task was filed for the buggy, the kiowa or the stock ATV: Untamed does not load them, and their files
are Unleashed's unchanged. The biplane and helicopter are Unleashed's alone, and MD80 already covers them.

## SLUS-21701's bike step against SLUS-21104's (MD81a)

Done statically, function by function, over the folded ELF (`GAME_REL.BIN` at 0x48ea80). The twins were
found by layout order, since the classes keep the same order shifted by about 0x18000, and by masked code
(`research/md81a/`: `align.py`, `near.py`, `twins.py`, `vtable.py`, `sdiff.py`, run through
`run_align.sh`). Then each pair was diffed with branch targets, `lui` values and field offsets set aside.
Addresses are in `research/oracle.txt # MD81a`. The bike object grew, so nearly every field moved:
`+0x10` from the wreck limits up, `-0x10` across the `0xb..0xd` block, `+0x30` at the chassis angles. The
wreck test's full map is in `crates/game-sim/tests/untamed_step.rs`'s `poke_wreck`.

| verdict | functions |
|---|---|
| the same code, fields moved | ground step, ground pitch torque, auto shift, air drag, unwreck, clutch |
| scheduling only (branch-likely, registers, moved fields) | ground head, balance, roll/pitch, stand upright, head/tail, end stunt, input limiter, peak rates, flip-wreck-steep (reason 15), wheel contacts (counts read `lhu`) |
| the wreck test | one change: a trick is a stunt id ≤ 48 (SLUS-21104: ≤ 40). A contact's flag is read as a halfword |
| the stunt-id bound only | start stunt (41 → 49), airborne control (41 → 49 and 46 → 54, three times) |
| reworked: new calls, constants, compares | backflip (1148 → 1292 B), spin in the air (2048 → 2104 B), preload (584 → 820 B), vehicle update (428 → 668 B), put back (320 → 424 B), wreck entry (288 → 308 B) |
| classified by MD93d | no-hander: scheduling only (FPU registers `$f5`/`$f6` swapped, one `mul/adda/madd` reordered to the same sum). Air state: reworked in one branch, out of the world (below) |

**Stunt ids shift by 8.** Every bound on the rider's stunt id moved by +8 (40 → 48, 46 → 54). That is
the eight stunts Untamed's table has beyond Unleashed's (`untamed_tricks`, 54 stunts). The port does not
compare ids with 40: `air::Step::stunt` comes from the rider's trick state (`rider.rs`), so nothing
changes there. Only `handling.rs`'s backflip `stunt_id > 40` has the number, and the backflip is reworked
anyway.

**The wreck test is proven.** `tests/untamed_step.rs` runs SLUS-21701's wreck test off an Untamed Free
Ride image (`context/untamed/ram/nat_or.bin`) on 10,000 generated states with Untamed's P limits, and
compares the reasons and throw with the port's `step::wreck_test` + `step::hit_wreck`. Every path is
covered (≥ 30 each), including 372 states with ids 41-48 mid trick. A second test shows that SLUS-21104's
bound answers otherwise on those ids. So the raised `[Wrecks]` limits are the whole of what Untamed changes
in the wreck test for a ride.

**The pack.** Untamed's sixteen `mx_*.phy.bpb` already ship in openmx-mods' `untamed_tricks` trick pack
(`import-tricks untamed`). A second copy in a machine pack would only duplicate them. So
`--handling mxvatv --handling-pack untamed_tricks` (or the Handling row with that pack) now rides them:
`trick_packs::Pack::handling` takes `mx_<bike>.phy.bpb` first, as SLUS-21701's loader does, and reads it
through `game_sim::bpb::ini`. Off by default, like all of MD40's handling.

**So an Untamed ride today** is SLUS-21104's step (MD40's blend) on Untamed's files. The wreck test is
bit-exact. MD93 has ported three of the six reworked functions (below); the other three are MD93a-c.

## Untamed's reworked functions, ported (MD93)

`--handling untamed`, or the Handling row's Untamed, is MX vs. ATV's handling on the same pack files
(`Handling::Untamed`, off by default) with SLUS-21701's reworked functions where they are ported, in
`crates/game-sim/src/handling/untamed.rs`. Each is proved by `crates/game-sim/tests/untamed_reworked.rs`
off the Untamed image, word for word over the bike and its body, with every call out hooked and compared
in order. Online, a room names only its pack, so an Untamed host's room rides as MX vs. ATV's.

| function | what changed from SLUS-21104 | port | in the ride |
|---|---|---|---|
| preload | an analogue preload: bound and pushed past 0.001 on the lip, it fires at once with the axis mapped onto `0..MaxPreload`, and the stick's path then runs in the same call. The cue is posted while firing (SLUS-21104: while the timer is above 0) | `untamed::preload` | yes, axis unbound (`None`; MD93e binds it) |
| wreck entry | reason 13's three random spins stored as one quadword, `w` zero; new vtable slots | `untamed::wreck_entry` | no (SLUS-21104's isn't either) |
| put back | the ground snap lifts the bike 0.3 ft along the normal and keeps the normal as up; the driver's respawn only with both first words set; no `+0x60`/`+0x70` restore; the attachment's put-back; post (8, 0x15) | `untamed::put_back` | no (the ride's put-back is the port's own) |
| backflip, spin in the air | — | MD93a, MD93b | — |
| vehicle update | a remote bike's (vehicle states 3/4) cleared stick has its angle and length worked out again (the arc cosine table's middle, and 0); otherwise SLUS-21104's on moved fields and slots | `untamed::vehicle_update` (calls as `untamed::UpdateCall`) | no (SLUS-21104's is hooked, not ported: `quad::Call::BikeVehicleUpdate`) |
| air state (1544 → 1524 B) | out of the world (the in-world test fails on the bike's origin): the event is posted under the moved tag, then the bike's **unwreck** (vt `+0x36c`, handed 1) runs, which ends its stunts and calls the put-back **without** the ground snap. SLUS-21104 clears the wreck reason and two more words and calls the put-back with the snap (vt `+0x3a8`, 1, 1). The rest is SLUS-21104's on moved fields: the bike's pointer moved to another register and the body's orientation is held in one, a 16 B larger frame | `untamed::air_state` (calls as `untamed::AirCall`) | no (SLUS-21104's isn't either: the ride's air step is `air::contact`'s) |
| no-hander (1328 → 1332 B) | nothing beyond scheduling: two FPU registers swapped and one `len²` sum reordered (the same `mula/madda/madd` result); its fields, table and callees moved | none needed | — |

**The air state and no-hander (MD93d).** MD81a's 171 differing lines in the air state were scheduling:
the bike moved from `$s0` to `$s1`, `$s0` holds the body's orientation pointer, and the frame grew by
16 B. With that masked (`sdiff.py` with the frame size and base registers set aside, the bike register
renamed), the callees were compared pair by pair. The balance clear, quaternion axes, in-world test and node
position are the same code on moved fields. The quaternion-to-euler conversion swaps its out-of-line `b`
stubs for branch-likely with the `mov.s` in the delay slot, which gives the same clamps. What remains is the
out-of-world branch (table above). The no-hander differs only in register allocation. `untamed::air_state`
is proved by `untamed_air_state_is_the_originals` (20,000 states; every call but the axes and euler angles
hooked and changing the state). Addresses are in `research/oracle.txt # MD93d`.

The brief's "every stunt-id bound +8" has nothing to do in these three: none of them reads the stunt id.
The bounds are in the wreck test (proved by MD81a), start stunt and airborne control (the port reads no id
there), and the backflip (MD93a).

## Not checked

- **Code** (MD81a compared the bike step, above; the other machines' steps are still unchecked). No function of SLUS-21701 was compared with SLUS-21104's. "Same classes" means the same
  `MXVehicle*.cpp` names and file formats, not the same step. The MD81 tasks capture first, for this
  reason.
- Whether the A/P `.phy` choice (A = arcade?) and the Race/Pro default are the same in Untamed's menus.
  `Vehicles.dat` unlocks `MX Pro Physics` and `ATV Pro Physics` from the start, where Unleashed locks them.
- `AIChrctr.aip` (AI riders) has no Unleashed file under that name and was not compared.
- The VehUI models, materials and textures (render data) were listed, not compared.
