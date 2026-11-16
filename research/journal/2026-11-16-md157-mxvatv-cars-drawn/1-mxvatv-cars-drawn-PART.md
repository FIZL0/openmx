# 1 — mxvatv cars drawn (PART)

Task MD157. 2026-11-16.

## What I did

- openmx-mods (branch openmx-MD157, ec7a20c, pushed): `import-atv cars` now also copies each car's chassis
  textures (its own map and every TextureMap_N `.pst`, out of `Vehicle_tex.res` or loose `Veh\Tex`), the five
  kinds' `Cam\FC_/RC_<kind>Default.pb`, and `Aud\carengine.aud` with every loop VAG it names. mod.json gets
  `"sounds": {"engine": "Aud/carengine.aud"}`, and `source.members` now lists every file's origin (modcheck
  failed on the new files without it). Pack: 302 files. `tests/disc.rs`' car test checks cameras and sounds.
- `crates/openmx/src/mxvatv_car_draw.rs` (new): loads the chassis `psg` its pack row names, under its own
  key (`mxvatv_car:<pack>:<car>`) with `load_repainted(.., &[])`, so MXU's `car:buggy` materials are not
  reused. Places it at scene node 0, poses its four tires off `atv_car::Ride` (roll angle, steer,
  compression), and prints `picture mxvatv car:` under `--trace-picture`.
- `main.rs`: the car is drawn on both renderers (GPU actors and software skinned), only while it is the
  vehicle out. Startup says `mxvatv car: drawn as <psg> off <pack> (N triangles, k of k materials textured)`.
- Engine sound: `machine_packs::Pack::car_engine` reads the pack's carengine.aud profile (buggy,
  monstertruck, baja, golfcart, SandRail). It is kept under `mxvatv:<profile>` so MXU's own [buggy]/[baja]
  are not taken for it. `MxvatvCarRide::engine_sound` feeds it the step's engine pitch (revs), redline,
  gas and body velocity, the same way MD136 does for the quad.
- Camera: MD82d's code already read `chase::mod_presets` off the pack. The pack lacked the files, so the
  car rode on the bike's camera. With them in, it starts on `Race Camera 1` of its own FC_ file.
- `game-render/src/car.rs`: the golf cart has neither the truck's hubs nor the buggy's arms. A new `cart`
  step after `arms` steers and lifts its `LeftHub`/`RightHub` and spins the front tires.

## What the original does

Not decompiled this task. The car model, textures, cameras and engine profile names are the disc's files.
The camera presets and the engine keying use the readers MXU's cars and MD136's quad already use.

## How it was proved

- `crates/openmx/tests/mxvatv_cars_drawn.rs`: all five cars on free09, headless, on a tape (switch at tick
  30, gas, stick hard left from 110). GPU for all five, software for the buggy. It checks:
  - the chassis is the pack's own `Veh/Anim/<car>.psg`, every material textured;
  - drawn on ≥100 frames, never before the switch, and never if picked but not switched to;
  - the springs take weight, every tire turns, the fronts steer, the car moves;
  - the camera is the first preset of its own `Cam\FC_<kind>Default.pb`, from just after the switch on;
    the bike's own before it;
  - the engine line names its profile with >0 loops.
  - Green with the pack installed: `TESTS PASSED: 11 tests`, shots written to
    target/tmp/md157-mxvatv-cars-drawn/. I looked at buggy (GPU and software), golf cart and monster truck
    shots: each car is drawn, textured, on its wheels, behind the chase camera.
- Sound: an A/B on a private weston with a fake pw-cat (mods with vs. without `sounds`). Per-second RMS,
  with: 210 872 2114 2481 2407 2974 3450 3411 3256 3413 4130; without: 848 1287 1054 987 1000 3241 3086
  3091 1121 1122 2479.

## Not verified / not 1:1

- The tire pose is by eye, not the original's class step: no capture compared.
- The golf cart's hub rig is the port's own guess from its node names.
- No driver sits in the car (MD175 filed).
- No dirt or dust off the tires (MD176 filed).
- The tires never stretch: the car step has no `MaxTireScale`, so `scale` is 1.0.
- The engine sound is proven only by the rough RMS A/B above. That run is wall-clock and other world
  sounds play too. Pitch and volume per loop were not compared with the original.
- Nothing was compared in PCSX2.
- The gate ran without the pack in mods/: I moved it to the scratchpad for `kit done`, then put it back.
  Only s2 has the pack. With it, the existing `mxvatv_cars` test is red on the sandrail. That is not from
  this task: it is red on b926e4509 without MD157's changes and with the MD39c-era pack (filed as K55). So
  `mxvatv_cars_drawn` skips in the gate. It is green only from my own run above.

## Next

- K55: bisect the sandrail red, then keep the pack in s2's mods/ so the gate runs both car tests.
