# 1 — map objects audit (FINAL)

Task V41. 2026-10-21.

## What I did

- **The train (and every path car).** `crates/game-render/src/toy.rs` ports the `.toy`'s `[CarN]` that run
  on a `.mot` clip: free06's train (`train_spline.mot`, 12 cars), dump trucks and scrapers, and the
  semi trailers of free09 and free11. The shell (`crates/openmx/src/main.rs`) loads them as `Traffic`,
  steps them on the scene's clock and draws them in both renderers. Commits 2a0f5e6 and 60426ec.
- **The cables: two bugs.**
  1. *Names.* The original's ini line reader drops every space, tab, CR and LF outside `[...]` and
     quotes before it splits a line at `=`. free06's scene names its high-tension wires
     `SLT=Free06_Power Line_A` … `_H` (18 placements). The original loads `free06_powerline_a.psg`;
     the port asked the disc for a name with a space, found nothing, and drew the pylons bare. free09's
     `SLT=Large Egg` (the World's Largest Egg) was lost the same way. Fixed in `scene.rs` (`slt_name`).
  2. *Root turn.* Many prop files have a root node whose local matrix is a rotation. In almost every
     case it is the placement's own rotation for one instance, left there by the exporter. The
     original draws a scene prop's vertices with the instance matrix alone. The port multiplied the
     root's turn in as well, which
     - laid free09's (and free06/07/11's) `Free09_PowerLine` sag *sideways*, so the wires ran
       ruler-straight from the road's view;
     - turned free10's `Wire01`…`Wire61` a second time off their poles;
     - stood free06/free10's `MM_HTTower` 60° off the line its wires hang from;
     - doubled the turn of 190 other placed models, among them free06's bridge and fences, free07's
       cable bins, wall and signs, and free10's runway.

     Fixed with `Assets::load_placed` (`world.rs`), which the shell (`main.rs`, `web.rs`) now uses for
     scene props. It sets a root node's local matrix to identity before the triangles are built, so
     both renderers draw from it.
- **The audit** of all 57 scenes the disc has (free06/07/09/10/11 and their `_ss`, nat01–22, sx01–24),
  from their `.scn`, `.toy` and `.list` files. The per-map list is below, and 8 follow-ups are filed:
  V42 (ecosystem trees), V43 (animated toys), V44 (birds and creatures), V45 (particles, sun flares,
  weather, toy sounds), V46 (the root rule for path cars), S80 (knockable props), S81 (car collision,
  sim clock, HasPhysics cars) and S82 (COL names with spaces).

## What the original does

- **Scene props.**
  - An `[ObjectN]`/`[ModelN]` line is read through the ini line reader, `./kit fn 0x0014a040`.
  - The model name is lower-cased and given `.psg`: `./kit fn 0x001a0360`, and the static model reader
    `./kit fn 0x001a8cd0`.
  - The archive lookup hashes the name by byte sum (`0x001f8760`) and compares it with strcmp
    (`0x001f8820`), so a name with a space never matches a file.
- **The instance matrix** is `M4::placement(pos, look, up)` (rows x = up×look, y = look×x, z = look,
  pos), read out of the drawn instances in RAM.
- **Path cars:** see `research/oracle.txt` § V41 for the car class's functions.
  - A car is a Hermite curve over the `.mot`'s Body channel, walked by arc length.
  - Each car lags the head by its `Lag` in feet.
  - Heading and normal are low-passed, and the car is snapped to the ground by a ground query.
  - The scene steps at 1/30 s.

## How it was proved

- **Save-state captures** of the original in PCSX2 copy 4:
  - free09 is the first human recording's state;
  - free06 is the biplane recording's state;
  - plus three scratch teleports on free09.

  The instances were found in RAM (`context/v41/*.ram`) by their positions.
- **`crates/game-render/tests/cables.rs`**, against the real disc:
  - free09's level and sloped `Free09_PowerLine` spans:
    - the placement rows equal the RAM instance rows to 1e-3;
    - the drawn model-space box equals the RAM instance box, centre (100,0,−3.992) and half
      (100.009,0.130,4.141), to 0.02 ft;
    - the span runs from (3119.265,201.616,2325.564) to the next pole 200 ft on;
    - it hangs 8.13 ft at the middle.
  - free06's 18 `Free06_PowerLine_*` placements:
    - all load;
    - `_A` at 4235.519,260.544,4304.064 has the identity rows the RAM shows;
    - its box is centre (6.852,3.352,−14.884), half (172.997,10.131,100.064);
    - the wire points within 40 ft of a pylon lie a median 0.50 ft from the pylon's drawn geometry
      (10.1 ft with the root turn drawn).
  - free10 `Wire03`: placed, its root is identity; in the file, its root is exactly its placement's
    turn.
  - Every model of all 57 scenes resolves to a `.psg` on the disc. None are missing.
- **`crates/game-render/tests/toy.rs`:** free06's train, trucks and scrapers, and free09's trailers,
  against their RAM poses (V41's earlier commits).
- **Looked right beside a screenshot.**
  - free09's wires before and after (`context/v41/shots/sag3.jpg`): before, flat lines at pole height;
    after, the catenary from pole top to pole top that the original shows in `pcsx2-high.png`.
  - free06's pylon with and without wires: `ht06.jpg`.

## Per-map special objects

"Port" means what this branch draws after V41. "Missing" means the port has nothing of it, in either
renderer. Every track's trees and bushes come from its `EcosystemFile` (`*.esb`), which the port does
not read (V42). The sx tracks have none (EcoDistance 0).

- **free06** (and `_ss`)
  - *Now right:*
    - high-tension wires `Free06_Power Line_A–H` ×18 on 7 `MM_HTTower`;
    - 156 `Free09_PowerLine` on 36 poles;
    - the train: 12 cars on `train_spline.mot`, `Free06_RR_Track_A–T` rails;
    - 4 dump trucks and 2 scraper pairs on their clips;
    - the bridge, rails, fences and ramps.
  - *Missing:*
    - 3 HasPhysics `ctruck` (S81);
    - crow flock `Flock01` on `loop_01` (V44);
    - `EyeBall` physics collection (S80);
    - 7 toy sounds, including `trainengine` (V45);
    - sun flare (V45);
    - ecosystem (V42).
  - *Collision:* cars none (S81).
- **free07** (and `_ss`)
  - *Now right:* 116 `Free09_PowerLine` on 33 poles; the turned `free07_cbnb/c/d`, `free07_wall`,
    `free07_sign2`, `free07radio3`.
  - *Missing:*
    - ranger_truck ×2 and watertruck ×2 (HasPhysics, S81);
    - Fire ×7 and Smoke ×20 emitters (V45);
    - BeachBall_B, Football and sign_* knockables (S80);
    - birds (V44);
    - Hits/Runs/MachineRace triggers (game-mode, not audited);
    - ecosystem (V42).
- **free09** (and `_ss`)
  - *Now right:* 156 `Free09_PowerLine` (sag); the `Large Egg` model; `Free09_Road` (loads and draws); signs; the water
    towers; the semi trailers on `thq_semi_truck(_02).mot` (V41 toy.rs).
  - *Missing:*
    - the rusticwindmill (HasCollision TRUE) and 4 seti_windmill animated toys (V43);
    - `free11_redtruck` (HasPhysics, S81);
    - birds (V44);
    - BeachBall, egg, barrels, cans, tool_box and sign_* knockables (S80);
    - the semi's toy sound (V45);
    - roadside trees (V42; plainly visible in the original beside the road).
  - *Collision:* `COL=Large Egg.col` ×2 dropped by the sim (S82).
- **free10** (and `_ss`)
  - *Now right:* Wire01–61 between 28 `Free10_ElectricPole` (double turn removed); the runway; 4
    `MM_HTTower` and `Free10_LargeWires`.
  - *Missing:*
    - `shovel_01` animated toy (collision) and `SH_windsock` (V43);
    - free10_truck ×4 (HasPhysics, S81);
    - washers, fridges, soda machines, barrels and crates knockables (S80);
    - birds (V44);
    - ecosystem (V42).
- **free11** (and `_ss`)
  - *Now right:* 52 `Free09_PowerLine` on 12 poles; the trailers on `car_loop_01.mot` (toy.rs);
    `Free11_Water`; signs.
  - *Missing:*
    - 8 balloons (V43);
    - ctruck ×2 and free11_policecar ×2 (HasPhysics, S81);
    - bowling pins, spare tires, earthball and sign_* knockables (S80);
    - 2 toy sounds (V45);
    - ecosystem (V42).
- **nat01**
  - *Missing:*
    - 4 rusticwindmill (collision, V43);
    - 12 birds with perches and land/takeoff/tunnel splines on triggers (V44);
    - hay bale collections (S80);
    - RainBegins/RainEnds weather triggers (V45).
  - *Static and drawn:* water tower, reservoirs, fences, signs.
- **nat02**: missing church_bell (V43), 9 vultures on triggers (V44), hay, barrel and crate knockables
  (S80). `HANGTREEROPE` and `Hangtree` are props and drawn.
- **nat03**: bridges and white rail fences drawn; hay knockables missing (S80).
- **nat04**: `Nat04_BIGBRIDGE`, `Nat04_Water` and 266 orange mesh fences drawn; 7 camels missing (V44).
- **nat05**: foot bridges, observation tower and rail fences drawn; cans missing (S80).
- **nat06**
  - *Missing:* 19 animated seti_windmill (V43); knockables (S80).
  - *Drawn:* 9 static windmills (`SETI_WINDMILL_STATIC` + `_BLADE_STATIC`) and the tracking facility.
- **nat07**: missing elk ×4 and mill ×3 sequence toys (V44), snowmen and bales (S80).
- **nat08**
  - *Missing:* gator_cook, pipe (V43); gators (V44); BlackSmoke ×5, BonFire ×3 (V45).
  - *Drawn:* `Nat08_STRING_OF_LIGHTS` ×27 props. Their `[Light]`s are not checked.
- **nat09**: missing BlackSmoke and BonFire (V45).
- **nat10**
  - *Drawn:* power_line/01/02/03 ×16, 40 telephone_pole and 151 tape_line on 49 posts. Their roots are
    identity, so V41's fix leaves them as they were. Not compared with the original.
  - *Missing:* knockables (S80).
- **nat11**
  - *Missing:* 8 gondolas (collision) running along `Nat11_LIFT_CABLE` (V43); WaterMist ×3 (V45).
  - *Drawn:* the cable, towers and terminals.
- **nat12**: missing the geysers (blast/mist/spray emitters on Geyser_a/b triggers) and MistPuff (V45).
  `Nat12_RAIL_A–E` and `Nat12_Water` are drawn.
- **nat13**: `Nat13_POWERLINE_A–D` ×13 and `Nat13_PWR_WIRES_A–G` are drawn (not compared with the
  original). Missing: fridge, soda, barrel and washer knockables (S80).
- **nat14**: missing BlackSmoke ×4 (V45) and hay (S80).
- **nat15**: missing steamsaw (collision, V43), BlackSmoke (V45) and knockables (S80). `TripleLightZone`
  is not audited.
- **nat16**: `Nat16_MINE_TRACK_A–D` ×40 and `DAMAGE_TRACK_A–D` ×19 are drawn. Crates are missing (S80).
- **nat17**: water and waterfall props drawn; missing BlackSmoke ×15, BonFire ×15, WaterMist ×9 (V45)
  and toucans (V44).
- **nat18**: missing ChimneySmoke ×6 (V45).
- **nat19**: missing 8 vultures on a trigger (V44).
- **nat20**: the finish arch, ice fences and Indian ruins have 60 collision objects named with spaces,
  which the sim drops (S82).
- **nat21, nat22**: fences and bridges drawn, nothing else special; nat22's hay is S80.
- **sx01–sx24**
  - *Drawn:* banners, flags, score and camera towers, lights and bridges as props.
  - *Missing:* the `FinishGate` and blimp animated toys (V43); Fireworks01–24 (V45); bale and banner
    knockables (S80).
  - *Not audited:* `TripleLightZone` start lights; the `[Light]` sections.

## Not verified / not 1:1

- **Path cars, wrap lag.** A lag of more than 500 ft past the clip's wrap sits 0.95 ft off the RAM
  pose. Reason: not explained. tests/toy.rs carries the tolerance.
- **Path cars, warm-up.** The cars start from the scene's clock with no warm-up history. This is my
  own choice, not proved against a fresh load.
- **Path cars, ground and normals.** The ground height is within 0.25 ft of RAM. The presampled
  normals were never compared with RAM.
- **Path cars, unported options.** PathIsLevel and DriveOnSoultree are not ported, because no audited
  car needs them.
- **Path cars, clock and collision.** The clock lives in the shell, not in the sim state, and the cars
  have no collision (S81).
- **Path cars, root turn.** They are still drawn with their file's root turn, and `train_car_03`'s is
  a 180° yaw. Whether the original drops the root for them is unchecked (V46).
- **free06's HT wires** are checked against the RAM instance matrix and box and against the pylons'
  geometry, not against a PCSX2 screenshot from the same spot. The biplane state looks away from
  them.
- **GPU renderer.** I did not compare its drawing of the wires with the original. Its frames before
  and after (sag.jpg, sag2.jpg) were from spots where no wire was in view. The software renderer's
  frames are the ones compared, and both draw from the same `Loaded`.
- **free10 wires.** The far ends of Wire09/10/11/48/49/60/61, which lead to buildings (service drops),
  are not checked.
- **Other props the root fix changes.** I checked the root rule on wires (free09, free10) and pylons
  (free06) only. The other 190 turned props (bridges, fences, runway, signs) were not compared one by
  one. The rule is the same and their root equals their placement's turn, which is the double turn.
- **Root translation.** No root node in any placed model has one, so dropping it is untested.
- **Collision objects.** The sim's `.col` placement may carry the same root issue. I did not check it,
  because the sim's lanes are proved against captures elsewhere.
- **Per-map list.** It is from the files, not from screenshots on every track. PCSX2 shots were taken
  on free06, free09 and free10 only, because a teleport and shot per track was not worth its cost when
  the files name every object.
- **free09's road.** In pcsx2-high.png the original shows a road below the wires, and the port's
  frame from the moment (high-after.png) does not. The two cameras are pitched differently, so the road
  may just be out of view; I did not check it.
- **Not audited:** `[Light]` sections, `TripleLightZone`, water materials' animation, `_ss` beyond
  their being copies of their base scene's objects, and the game-mode triggers.

## Next

Nothing for V41. The follow-ups are V42–V46 and S80–S82.
