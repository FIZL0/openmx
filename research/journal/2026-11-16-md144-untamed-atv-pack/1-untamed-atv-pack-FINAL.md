# 1 — untamed atv pack (FINAL)

Task MD144. 2026-11-16.

## What I did

- **The routing (game-sim, `quad.rs`).** `quad::Ride::untamed()`, called after `rig`, puts the quad on
  Untamed's step. The one switch is the scene's `untamed` flag (`Bike::untamed`), so no `Parts`/`Bike`
  literal had to change. With it on:
  - preload calls `handling::untamed::preload` (MD93);
  - the backflip calls `handling::untamed::backflip` (MD93a): R1 arms it with other buttons held, and the
    flight's torque decays only with no trick button held;
  - the 360 (`SpinAir`) calls `handling::untamed::spin` (MD93b), in a new `Steps::spin_untamed`;
  - airborne control gets the rider's stunt id − 8 (below);
  - the hazard clock is `Tail::untamed` (MD81b), and the node update is `Scene::update_untamed`, through the
    scene flag;
  - the body integrates with `Body::integrate_untamed` (MD81c's per-time angular damping), through a new
    default method `Moto::untamed`. Only `Steps` answers yes;
  - the stunt ids are Untamed's: backflip 49, 360s 52 and 53.
- **The pack (openmx-mods, branch `openmx-MD144`, pushed, stacked on `openmx-MD81f`).**
  - `import-atv untamed-atv` copies SLUS-21701's `res\veh\VehicleType_1.res` whole into
    `untamed_atv/` (59 files).
  - `mod.json` says `"machine": "atv"` and `"step": "untamed"`. Its `phy` lists `Veh/atv_Race250P.phy.bpb`
    and `Veh/atv_Pro250P.phy.bpb`. Its one build is `ATV_R.mcf` / `250r_atv_chasis` /
    `ATV_R_250_KTM.sub.bpb`.
  - build.sh's SLUS_217.01 line now builds it beside `untamed_cars`.
- **The shell (openmx).**
  - `machine_packs::Pack` reads the pack's `step` (`Pack::untamed`).
  - `AtvRide::read` reads a `.phy.bpb` through `game_sim::bpb::ini`. `AtvRide::on_untamed_step` makes
    `made` call `ride.untamed()` after `rig`. `atv_stem`/`atv_label` drop `.bpb`.
  - `--atv atv_Race250P@untamed_atv` picks the pack. A bare stem picks the first quad pack not on Untamed's
    step, so it stays `mxvatv_atv`'s. The online reader (`quad_reader`) honours the pack's step too.
  - The machine picker offers an Untamed pack's quads as `STEM@PACK`, named "Race 250 P (Untamed)", so the
    Quad row tells them from Unleashed's. `Menu::quad_pick` hands the lobby the bare stem.
- Installed `untamed_atv` and `mxvatv_atv` into this checkout's `mods/machines/` (git-ignored). Unzipped
  MD147's `spawn.p2s` and `air.p2s` to `context/untamed/md147/{spawn,air}.ee.bin`.
- Filed MD144a (classify the unreached callees) and MD144b (draw the pack). The replay half was already
  MD168.

## What the original does

- Untamed's quad step is SLUS-21104's on moved fields (MD81b). What differs on its path is the code it
  shares with the bike: preload, backflip, 360, the hazard clock, the node update and the body integrate.
- **Airborne control's bounds.** SLUS-21701 compares the rider's stunt id with 49 and 54 where SLUS-21104
  has 41 and 46. MD81b wrote that "the port does not read" them. It does: `handling::airborne_control`
  tests `stunt_id < FIRST_FLIP_STUNT` and `>= IDLE_STUNT`. Untamed's ids are all SLUS-21104's + 8. MD147's
  spawn image has the quad's backflip id 49 at `+0x13c8`, the 360s 52 and 53 at `+0x13cc`/`+0x13d0` (and
  50, 51 at `+0x13d4`/`+0x13d8`). So handing the shared function the id − 8 is exact for both compares.
- **The two 360 stunt ids.** SLUS-21104 starts `stunt_left` (45) for a direction over 0, else `stunt_right`
  (44). Untamed's `spin_stunt` gives 53 for a direction ≥ 1, else 52. With +8 these are the same choice.
- Untamed's `VehicleType_1.res` has only the race chassis (`250r_atv_chasis.psg`, `ATV_R_Chasis*.col`) and
  the four race `.phy.bpb` (Race/Pro, 250A/250P). Both listed quads weigh 420.

## How it was proved

- **Generated states, routed against SLUS-21104's path** (`crates/game-sim/tests/untamed_atv/route.rs`):
  - `the_quads_untamed_spin_and_backflip_are_slus21104s_where_both_agree`: 40,000 states, half 360 and half
    backflip. They are built off the R quad made from Unleashed's `atv_race250p.phy` (MD123's files). Each
    state has at most one trick button, R1 if any, the stick off centre, and a stunt id outside 41..48. Those
    are the states where the two originals do the same. Every word of the quad, its body, its steps' words,
    and the calls handed on are equal routed or not. Of the states, 5,742 handed a call on, 8,085 turned a
    360 or flip on or off, and 2,381 moved the body.
  - `the_quads_untamed_step_arms_on_r1_with_other_buttons`: with R1 held beside another button, 20,000
    states. Some come out different for both the 360 and the flip, so the routing is live.
  - `the_quads_untamed_air_control_reads_the_stunt_id_8_lower`: 4,000 airborne states with ids 36..65. The
    routed call on id n equals the SLUS-21104 call on n − 8.
  - `the_untamed_quads_stunt_ids_are_its_spawn_images`: `Ride::untamed`'s ids (49, 52, 53) are MD147's
    spawn image's, and the image's quad is `atv_Race250P.phy`.
  - Untamed's own functions were already proved against SLUS-21701 by MD93/93a/93b (bike), MD81b (clock,
    node update) and MD161 (`integrate_untamed`).
- **Pack off:** game-sim's `atv_quad` (120 tests, every C28 replay) passes. The switch defaults off, and a
  bike ride with the Untamed quad picked but never switched to hashes the same as one without.
- **Shell** (`crates/openmx/tests/untamed_atv.rs`, ran here without skipping):
  - `the_untamed_quad_rides_in_the_shell_on_untameds_step`: the line says "on MX vs. ATV Untamed's step".
    The Race quad goes 479 ft on the gas, and the Pro quad rides too. The ride hash is unchanged when it is
    picked but not switched to.
  - `a_bare_quad_is_unleashed_s_beside_the_untamed_pack`: a bare `atv_Race250P` is `mxvatv_atv`'s, and its
    line differs from the Untamed quad's.
- **openmx-mods:** `import-atv`'s ignored disc test `the_untamed_quad_pack_is_the_discs_and_openmx_reads_it`
  passes. modcheck finds every member byte for byte against the disc, and each listed quad builds off the
  pack's chassis through openmx's readers.

## Not verified / not 1:1

- **MD147's rides are not replayed** (the task's replay half). That needs an Untamed sim-frame harness:
  Rocksbury's terrain, the frame's hooks at SLUS-21701 addresses, and an image reader at Untamed's offsets.
  It is MD168, filed before this task. So the routed quad has not been compared with the original over a
  whole ride.
- **The unclassified callees** (`.phy` tire reader, put-back, animate, node pose, net unpack, wreck path)
  are not classified. That is MD144a. Any of them may hide a rework the routed quad lacks.
- **The routing is proved only where the two originals agree.** Where they differ (R1 with other buttons,
  ids 41..48, the decay's no-button test), the routed path is Untamed's function as proved on the bike,
  but the quad's wiring there was not held against SLUS-21701 itself.
- **`spin_untamed`'s inputs.** The flick hold (`hold`) is the quad's spin `flick` word, and the forward
  axis is the scene's axis 0 read only while a 360 is active (as SLUS-21104's). `button` is `None`: the
  quad's pad has no bound 360 control. Rate increase is 0, since the quad's 360 keys have none. These
  match SLUS-21104's wiring, but no Untamed quad capture checked them.
- **The backflip's lip stick and hold** are the quad's `stick.1` and `stick_hold`, as SLUS-21104's are.
  This was not checked on an Untamed capture.
- **`integrate_untamed` on the quad** is routed but not exercised: the race quad's damping lanes were not
  checked for values under 1, and no quad test lowers one.
- **Pro250P** has no capture. Only the Race quad is in MD147's images. The Pro quad is shown only to build
  and ride in the shell.
- **The pack is not drawn.** Its `.sub` is a BPB, and it lacks a rider set, so `quad_draw::load` says
  "undrawn". It has no cameras, sounds or particles read either. That is MD144b, and the pack's `todo`
  says it. It was not looked at beside Untamed (T7).
- **The shell rides the Untamed quad on MX Unleashed's worlds**, with the map-edge gate as those worlds set
  it (MD163 found Rocksbury leaves it 0).
- **The pack is on openmx-mods' `openmx-MD144`**, stacked on the unmerged `openmx-MD81f`/`MD81e`/`MD163`/
  `MD39c`, so it reaches main with them.

## Next

MD168: seed the port's quad from `context/untamed/md147/spawn.ee.bin` with `Ride::untamed()` and replay
`steer.csv` first. Then MD144a's callees, if the replay parts.
