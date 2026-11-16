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

## SLUS-21701's ATV step against SLUS-21104's (MD81b)

Static, as MD81a's: twins paired by layout order over the folded ELF (GAME_REL.BIN at 0x48ea80), each pair
disassembled at the larger of the two Ghidra sizes with `./kit dis --limit 100000` (MD81a's own diffs ran
under the 400-line default and may have been cut short on long functions), compared by `sdiff.py`, then the
`jal` callees paired in order and walked three deep from the quad's ground step, ground head, air state,
update and wheel contacts. Addresses are in `research/oracle.txt # MD81b`.

The quad's fields moved: −0x10 through the `+0xb..+0xd` block (vehicle state `+0xbc4` → `+0xbb4`, wreck
words `+0xc80` → `+0xc70`, hazard words `+0xc10`/`+0xc14` → `+0xc00`/`+0xc04`, the clock `+0xdf0` →
`+0xde0`), +0x10 for the engine (`+0x142c` → `+0x143c`), +0x40 for the wheels (`+0x17b0..bc` →
`+0x17f0..fc`), +0x30 for the chassis angles and the body (`+0xb8` → `+0xe8`), `+0x704..710` → `+0x728..734`,
`+0x1744/48` → `+0x1784/88`, the run `+0x680` → `+0x6a4`. The bike-class slots the quad calls are 8 lower
(`+0x4b8` → `+0x4b0`, `+0x4ac` → `+0x4a4`, `+0x4d0` → `+0x4c8`, `+0x53c` → `+0x530`).

| function | SLUS-21701 against SLUS-21104 | port |
|---|---|---|
| ground step | the same code | — |
| ground head, stand upright, roll/pitch, pitch torque, wheel contacts | moved fields, a moved slot, scheduling; the contact counts read `lhu` (as the bike's) | — |
| air state (the quad's) | `$s0` → `$s1`, a 16 B larger frame; the same signature and float ops; no vtable call, so none of the bike's out-of-world rework | — |
| wheel suspension, tire forces, wheel integrate, rev limit, wheel clear, peak rates, clutch, auto shift, air drag, contact velocity, hinge force, steer and spin integrate, quaternion axes, ground query, terrain cell, node rotate/unplace, curve value | the same code (branch-likely and scheduling only) | — |
| airborne control | `slti 41` → `49`, `46` → `54`: the trick bounds on the rider's stunt id. The port does read them (`handling::airborne_control`'s `stunt_id`); Untamed's ids are SLUS-21104's + 8 (MD147's spawn image: backflip 49 at quad `+0x13c8`, 360s 52/53 at `+0x13cc`/`+0x13d0`), so both compares come out the same | MD144: the quad hands it its stunt id − 8 (`quad::Steps`, `untamed_atv.rs` `the_quads_untamed_air_control_reads_the_stunt_id_8_lower`) |
| preload, backflip, spin in the air (the bike's, called by the quad's ground step) | reworked, as on the bike | MD93, MD93a, MD93b: `handling::untamed::{preload, backflip, spin}`; MD144 routes the quad's calls to them (`quad::Ride::untamed`) |
| update | the bike's vehicle update is Untamed's (MD93c); the hazard clock is called straight, not through vt `+0x45c`, with a **third word**; the rumble pad's slots moved (`+0x1c`/`+0x4c` → `+8`/`+0x40`); the distance's `if d < 0 { d = 0 } dist += d·dt` became `if !(d <= 0) { dist += d·dt }` (the same sum but on a `-0` distance; below) | `quad::Tail::untamed` |
| **hazard clock** | handed `grounded` (any wheel's `+0xd0`, the port's `wheel_sunk`) beside `on`. With it clear, the free reset is passed over: the vehicle is not free, stuck unless the run's flag says not, and the clock runs by the surface time (−1 put back at once, ≤ 0 time zeroed, > 0 counted and put back past it). With it set, SLUS-21104's | `handling::untamed::stuck`, proved by `untamed_hazard_clock_is_the_originals` (4000 states) |
| **node world update** (the scene graph's, every machine's) | the topmost node brought up to date, when it has a parent, has its new world frame made orthonormal again (a new call: the first row the second × the third, the second the third × that, both normalised by `rsqrt`, the first their cross). The node's words moved +0x30 | `atv_moto::Scene::update_untamed`, `atv_moto::untamed_orthonormal`, proved by `untamed_node_world_update_is_the_originals` (2000 three-node chains) and `untamed_orthonormaliser_is_the_originals` |

The distance clause: SLUS-21104 raises a `d` under 0 to 0 (`c.lt.s`; a NaN or `-0` is kept and added),
Untamed adds only when `!(d <= 0)` (a NaN is added too, a `-0` not, and `dist + -0·dt` is `dist`). They part
only on a negative `d` with the distance `-0`: SLUS-21104's `-0 + +0` is `+0`, Untamed's stays `-0`. The stat
starts at `+0` and only adds, so it is never `-0`. Not ported.

**On the quad since MD144.** `quad::Ride::untamed` (called after `rig`) sets the scene's `untamed` (the node
update is `update_untamed`), `Tail::untamed` (the hazard clock), the stunt ids (+8), and the quad's steps
then call Untamed's preload, backflip and 360, hand air control the id − 8, and integrate the body with
`integrate_untamed` (MD81c's per-time angular damping). Off by default: every replay and hash is
SLUS-21104's. The routing is held to the SLUS-21104 path on states where the two originals agree, and to
differ where R1 is held with another trick button (`tests/untamed_atv/route.rs`); no Untamed ride is
replayed through it yet (MD168). openmx-mods' `untamed_atv` pack (`import-atv untamed-atv`) lists
`atv_Race250P`/`atv_Pro250P` `.phy.bpb` with `"step": "untamed"`; the shell rides it as
`--atv atv_Race250P@untamed_atv`.

**Captured (MD147), not ported.** Vehicle ATV with Physics Type RACE is `atv_Race250P.phy` (named at quad
+0x134; vtable 0x003f0470, kind 1). MD147 kept spawn and in-air states on Rocksbury and seven rides at the
moved offsets (research/journal/2026-11-15-md147-untamed-atv-capture, research/md147): throttle twice (the
same ride; the rows that differ are late or torn reads), steer, a jump with a wreck, a landing from the air
state, and two hazard rides. **Rocksbury's hazard table is all zero** (quad +0xde4, 16 entries of hazard
word and time, indexed by each wheel's surface byte +0x110), so the hazard rides poke it. With hazard 0 and
time 1, the clock counts in the air only (every wheel's +0xd0 clear) and puts the quad back on the tick
it passes 1 s: Untamed's new branch, which SLUS-21104 never takes. With hazard 1 the quad is put back to
the spawn every second, and its rear wheels are off the ground for a few ticks after each drop. The
stick moved −0x10 (+0x1298), the ramped stick +0x10 (+0x12f8): not every 0x12xx word follows one rule.

**Put-back on Untamed's step (MD168a).** The quad's put-back resets each wheel (contact, compression,
spring, spin and drive words, normals to up), detaches, and runs the vehicle put-back: the saved spot
(`+0xd30`, SLUS `+0xd40`) snapped to the ground and lifted 0.3 ft along its normal, facing the saved
forward, then the ground reset (orient, body commit: still, at rest). The spot is kept by a keep-spot slot
(vt `+0x3ac`) each tick: the last-free place (`+0xbf0`, written at the tail's start while free) when stuck,
else the body; only with no wheel on a hazard (`+0xc10`) and a ground normal y over 0.8, the forward
flattened. So hazard-time goes back to where it left the ground, hazard-on always to the spawn.
`quad::Ride::tail_on_untamed` (when `Tailed::untamed` is set, by `Ride::untamed`); MD147's hazard-time
replays bit for bit to row 190 and hazard-on through all 370 rows. The put-back rows look a tick late
only because the capture reads before the tick's tail.

Not classified (they differ, but are world, collision or presentation code the quad shares with every
machine, or were not reached): the plant's axis-angle turn (628 → 312 B: likely a Ghidra size split), the
quaternion-to-euler (MD93d found the same clamps; a new `jal` past its end at the larger size), the collide
against search (slots `+0x11c`/`+0x120` → `+0x13c`/`+0x140`, counts `lhu`), the pair accept (`lhu`), the eco
part (a `beq`/`bne` swap), the event post (`lhu`), two unnamed callees of the update, the name bit (its callees
did not pair), and the quad's `.phy` tire reader, put-back, animate, node pose, net unpack and wreck path,
which the walk did not reach.

## SLUS-21701's car step against SLUS-21104's (MD81c)

Static, as MD81a/b's: the monster truck's vtable (SLUS-21104 and SLUS-21701 side by side) paired its
slots, the rest was paired by layout order (car code about +0x1d9d0 higher in Untamed, truck code about
+0x18000) and each pair compared by `research/md81c/fp.py`, a fingerprint blind to order: the function
cut at the first prologue after its `jr $ra`, branch-likely folded into the plain branch, nops dropped,
then the op multiset, the float `lui` constants, the small immediates and the call count compared. The
ones it called different were read by eye (`./kit fn`, `./kit dis --limit 100000`). The loaders' key
lists were diffed by `research/md81c/keys.py` (it resolves each `lui`/`addiu $a1` pair to its string).
Addresses are in `research/oracle.txt # MD81c`.

The car's fields moved −0x10 through the `+0xb..+0x12` block (state `+0xbc4` → `+0xbb4`, wreck words
`+0xc80` → `+0xc70`, the tires `+0x1254` → `+0x1244`, the yaw limit `+0x124c` → `+0x123c`), the body
pointer `+0xb8` → `+0xe8` and the `+0x3xx` words +0x30; the node's words +0x30 (as MD81b's). The car-class
slots from about `+0x45c` on are 8 lower (`+0x488` → `+0x480`, `+0x47c` → `+0x474`, `+0x484` → `+0x47c`).

| function | SLUS-21701 against SLUS-21104 | port |
|---|---|---|
| truck PreProcess, Force, travel centre of gravity, anti-slide, launch, aero, PostProcess (the truck's); engine, shift; tire step (and a part), tire clear, probe; wreck clock, wreck and unwreck, respawn; ghost step, base update; the body's linear half, end of substep, local force/moment, add force, set centre of gravity; ground query; node local and position | the same code, fields moved | — |
| controls, two tire-step parts, contact sort, vehicle post (and its placement), world step, add moment | scheduling, registers, `lhu` counts | — |
| loaders (car, base, truck) | the same 239, 6 and 4 keys with the same float defaults; the reader is handed as file+60 | — |
| PostProcess (the car's) | the yaw clamp rewritten: SLUS-21104's `!(abs(r) <= L) → r = L·sign(r)` became `r > L → L`, `r < -L → -L`. The same for any normal `L` (and a NaN `r` is kept by both); they part on a denormal `L` only | `atv_car::post_process_untamed` (MD161, proved by `tests/untamed_truck.rs` as a stretch) |
| **input step** | reads the controls block (stick bytes, per-command strengths) in place of the pad record; the pad word is player 0's lookup of the pad command; every command lookup gains a "−1 = unbound → not pressed" guard; the hand brake is the next command id and tests `<= 0`; the clutch is `s <= 0 ? 0 : 1` (SLUS-21104's `s == 0 ? 0 : 1`: they part on a negative strength only) | the clutch: `atv_car::input_untamed` (MD81d); the guards need no code (the shell hands the port strengths, an unbound command's is 0); the hand brake is the port's `HAND_BRAKE` tested `<= 0` either way |
| update (the car's) | the hazard clock called straight with the grounded word (any tire's `+0xa4`), as MD81b's quad | `handling::untamed::stuck` (MD81b) |
| class update (the truck's) | the odometer's clause as MD81b's distance (`!(speed <= 0)`): parts only on a `-0` odometer, which never happens | not ported |
| vehicle force | after the gravity, a second body add of `(0, 0, 0, 0)`: changes nothing but a `-0` or denormal component | not ported |
| preload | the state logic is the same; the cue posted while firing, as the bike's (MD93). The truck's `[Preload]` is off, but the cue still posts when it fires with timer 0: only the port's `cued` latch | `Keys::untamed` (MD81d) |
| class step (the truck's) | model posing (scene-node calls) only; Untamed adds four node calls. Not ported for SLUS-21104 either | — |
| **body integrate** (every machine's rigid body) | the angular damping is made per-time: with `h > 0`, `k = 0.333/h` (the literal is `0x3eaa7efa`, 0.333f); if `abs(k − 1) > 0.1`, each damping lane becomes `powf(d, k)`, else `d` as is; with `h <= 0` a rodata vector. Then the same `rates = d·rates + h·accel` and per-axis clamps as SLUS-21104's. The quaternion step works on a copy before normalising (the same values) | `atv_body::Body::integrate_untamed` on the car ride (MD161, proved by `tests/untamed_truck.rs`); not yet on Untamed's bike and ATV rides |

**The body-integrate change is not on a plain truck ride.** The truck's `DefaultAngularRateDamping` and
`ImpactAngularRateDamping` are `1, 1, 1` (unchanged from SLUS-21104's file) and fdlibm's `powf(1, y)` is
1 for every `y`. It shows the moment something lowers a lane below 1: the contact pass's collider damping
(`contact.rs`, `damp_angular`), a ragdoll's, or a `.phy` that sets it. Then `powf(d, 0.333/h)` at the
substep `h` damps far harder than SLUS-21104's `d`. It touches the Untamed bike and ATV rides too
(MD81a/b did not reach it). The port's Untamed bike ride does not take it (MD161c): it integrates MX Unleashed's
chassis (`chassis::integrate`, no angular damping lanes), not `atv_body::Body`, so there is nothing to route
until a Handling bike ride runs on that body (MD175). Untamed's powf (the thunk's target, 2032 B) has fdlibm's shape (its zero
tests read `< 0x00800000`, as SLUS-21104's) but was not proved against `ps2_float`'s.

**Captured (MD160), not ported.** MD160 drove the Untamed menus by tape and kept spawn and in-air states
and seven truck drives (research/journal/2026-11-14-md160-untamed-monster-capture). Its wreck drive shows
the powf damping live: ×0.85219 per tick in the air from a stored 0.999. The port of the four live changes (input guards,
clamp form, preload cue, body damping) behind an Untamed switch, and the openmx-mods pack are follow-ups.

**Ported (MD161).** `atv_car::Load::untamed` now also takes the body's per-time damping
(`atv_body::per_time_damping`, `Body::integrate_untamed`) and the PostProcess's yaw hold by compares
(`atv_car::hold_yaw_rate_untamed`) through `atv_car::frame_untamed`. `tests/untamed_truck.rs` proves both
against the original over the RAM image: the integrate and end-of-substep over 20,000 bodies (damping lanes
1, 0.999, 0, `-0`, denormal, above 1, negative; `h` at 1/120, 1/115, 1/60, 1/240, inside and at the edges of
the `abs(k − 1) <= 0.1` band, 0, `-0`, negative), which proves Untamed's powf along the way; the yaw hold as a
stretch of the PostProcess (the vehicle post stubbed). The one lane not 1:1: on the raised path the
original's `w` is a stack word its integrate never writes; the port keeps the damping's `w`. A machine
pack's car entry with `"step": "untamed"` rides on it (`--mxvatv-car CAR@PACK`). The replay of MD160's
drives and the pack itself are follow-ups (see `plan/`).
The truck's `.phy` changes themselves (Weight, GravityScale, FrictionZone1, `[Preload]` off) need no code:
the loaders read the same keys.

## The hazard clock's callers and the node update on every machine (MD145)

Untamed's hazard clock (MD81b) has three callers, each handing its own `grounded` (addresses in
`research/oracle.txt # MD145`):

| caller | `grounded` | `on` | port | proof |
|---|---|---|---|---|
| bike update (twin of SLUS-21104's Motocross override) | either wheel's ray down (`+0xd0`) | it and the held word (either wheel's surface holding) | `handling::untamed::bike_hazard`, in `handling::tail::vehicle_update_untamed` | `untamed_bike_hands_the_clock_either_rays_hit`: a stretch of the update, the clock run, 6000 states |
| car update | any tire solidly down (`+0xa4`) | it and some tire on a hazard surface | `atv_car::Update::untamed` (MD81d) | `untamed_car_hands_the_clock_any_tire_solid`: a stretch, the clock's arguments against the port's call, 6000 states |
| quad update | any wheel's ray down (`+0xd0`) | it and the hazard surface | `quad::Tail::untamed` (MD81b) | MD81b's static read |

Beside the clock, Untamed's bike update changes the rumble pad's slot (presentation) and the run's distance
clause, as the quad's: `if !(d <= 0) { dist += d·dt }`. `vehicle_update_untamed` takes both. Its inner update is
still SLUS-21104's `tail::inner`; Untamed's is `untamed::vehicle_update` (MD93c), with another shape.

**The `Handling::Untamed` bike ride has neither.** `ride.rs` rides MX Unleashed's sim with MX vs. ATV's steps:
it has no hazard clock (its put-back is its own) and no scene graph. `tail::vehicle_update` runs only in the
SLUS-21104 frame tests (`tests/mxvatv_bike_ride.rs`). So nothing on that ride takes SLUS-21104's clock or node
update, and there is nothing to swap. The Untamed versions are ready for a ride of the MX vs. ATV bike's own
frame.

**The node update.** `atv_moto::Scene::untamed` makes `Scene::update` Untamed's (`update_untamed`), so every
scene method that brings a node up to date goes through it. `atv_car::Load::untamed` sets it with the other
switches. The quad's scene takes it with the quad's other Untamed routing (MD144). Untamed's change only
touches the topmost stale node **with a parent**. In MD164's trophy-truck spawn and air images, the truck's
node has no parent, and the port's car scene holds the truck's node alone, as a root. So on a car ride the
switch changes no word (`untamed_car_node_has_no_parent`).

**MD81c-f's trucks** (monster truck, baja, golf cart, sandrail: all the car class) need the car's clock,
which MD81d routed, and no more: they share the car update, so `grounded` is any tire's `+0xa4` for all four.
They need nothing from the node update while their scene is the truck's node alone. It would matter only
when a truck's model nodes (tires, body parts under the truck's node) come into the port's scene, because
each of those has a parent.

## Not checked

- **Code** (MD81a compared the bike step, above; the other machines' steps are still unchecked). No function of SLUS-21701 was compared with SLUS-21104's. "Same classes" means the same
  `MXVehicle*.cpp` names and file formats, not the same step. The MD81 tasks capture first, for this
  reason.
- Whether the A/P `.phy` choice (A = arcade?) and the Race/Pro default are the same in Untamed's menus.
  `Vehicles.dat` unlocks `MX Pro Physics` and `ATV Pro Physics` from the start, where Unleashed locks them.
- `AIChrctr.aip` (AI riders) has no Unleashed file under that name and was not compared.
- The VehUI models, materials and textures (render data) were listed, not compared.

## SLUS-21701's car step against SLUS-21104's (MD81d)

Static, as MD81a/b's, done beside MD81c (the monster truck, branch `task/MD81c`, same day) and agreeing
with it function for function. The baja's object is the buggy's class (vtable `0x003cec60` in SLUS-21104),
so its world step, update, class update and input step were paired by layout order and compared by
`research/md81d/opdiff.py`: a mnemonic-only diff that folds branch-likely, drops nops, moves, `$sp` and
64/128-bit loads and stores, keeps `$zero`-based immediates and cancels a deletion matched by an equal
insertion elsewhere. Each span runs to the next function's start (Ghidra's sizes were short). Addresses are
in `research/oracle.txt # MD81d`.

| function | SLUS-21701 against SLUS-21104 | port |
|---|---|---|
| controls, travel cofg, PreProcess, aero (rodata equal), anti-slide, Force; tire step and its parts, probe, tire clear; engine, shift; vehicle pre/post; ground query | the same, fields moved (register renames, frame shifts, pointer reloads kept in registers) | — |
| contact sort | the count read `lhu` | — |
| buggy/baja class update | the distance clause `!(d <= 0)` (as MD81b's quad; parts only on a `-0` distance) and an assert's line | not ported |
| PostProcess | the yaw clamp by two compares (as MD81c: the same for any normal limit) | `Load::untamed` → `post_process_untamed` (MD161) |
| **preload** | the cue posted while the push is on (`+0xc44`), not while its timer is above 0 | `Keys::untamed`, proved |
| **vehicle force** | a second body add-force of a zero quad after the gravity | `Vehicle::untamed`, proved |
| **update** | the hazard clock called straight with the grounded word (any tire's `+0xa4`) | `Update::untamed` → `handling::untamed::stuck` (MD81b) |
| **input step** | reworked (MD81c's row): the clutch `<= 0 ? 0 : 1`, unbound commands read as not pressed, the controls block's stick and strengths, the pad word off the preload command | the clutch only: `atv_car::input_untamed` |
| body integrate | MD81c's per-time angular damping (`powf(d, 0.333/h)`) | `Load::untamed` → `integrate_untamed` (MD161); on a plain baja ride the keys are `1, 1, 1` in both files and `powf(1, y)` is 1 |

**Where the preload parts.** With `PreloadTime` 0 (Untamed's baja has `[Preload]` off) a fired preload
is active with its timer 0: Untamed posts the cue, SLUS-21104 does not. And while armed the timer winds
above 0: SLUS-21104 posts the cue while winding, Untamed only once it fires. The state words are the same.

**Switched together.** `atv_car::Load::untamed` (false as read) sets the three switches in `Load::keys`,
`Load::vehicle` and `Load::ride`, and `Ride::pad` takes Untamed's input step with it. Nothing sets it yet,
so every replay and hash is SLUS-21104's. The node world update (MD81b's `Scene::update_untamed`) is not
routed for the car, as it is not for the quad: the scene's methods call `update` inside.

No Untamed baja drive was captured and there is no pack: see the journal
(`research/journal/2026-11-14-md81d-untamed-baja`) and its follow-ups.

**Captured (MD164), not ported.** The menus' TROPHY TRUCK is the baja: its vtable holds the buggy/baja
class update and its kind is 6. MD164 kept spawn and in-air states and eight drives
(research/journal/2026-11-14-md164-untamed-baja-capture): throttle twice (identical, raw blocks included),
steer (it tips over by itself: car state 14, damping 0.999), brake, a jump through all 4 gears, a landing
from the air state, and a **preload fire**: armed clears and the timer goes to −1/120 with a zero push, and
the body stays as the jump's without it. The cue latch rises and falls inside the tick, so the cue post is
inferred from the state words, not seen.

## SLUS-21701's golf cart against SLUS-21104's (MD81e)

The golf cart class (vtable `0x003cf600` → `0x003f1370`) keeps SLUS-21104's code for its own slots:
destructor, node finder, `+0x354`, `+0x37c` and the class step are the same; the loader reads the same 11
keys (MD81c's reader change only); `+0x3dc` and its thunk move fields by the layout rule. Its class update
is the baja's (SLUS-21104's golf one was the buggy's), so it carries MD81d's `!(d <= 0)` clause, not
ported. Nothing else to port: `atv_car::Load::untamed` and Untamed's golfcart.phy (`MaxAnalogThrottle`
0.1, `[Preload]` off). Addresses in `research/oracle.txt # MD81e`; journal
`research/journal/2026-11-15-md81e-untamed-golfcart`. With `MaxAnalogThrottle` 0.1 any counted press is
the whole gas: the capture shows no gas through pressure byte 23 and all of it from about 25.
