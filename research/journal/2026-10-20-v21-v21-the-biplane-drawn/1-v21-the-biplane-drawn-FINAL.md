# 1 — v21 the biplane drawn (FINAL)

Task V21. 2026-10-20.

## What I did

- `crates/game-render/src/biplane.rs`: the plane as the renderer needs it. `Biplane::load` loads one of
  the four liveries (`biplane`, `biplane2`, `biplane_ss`, `biplane2_ss`, the spawner's order) and both
  prop cones (`propconeblur`, `propconebent`) out of `machine.res`, and keeps `PropDummy`'s transform and
  the four seat anchors (`Dummy_Foot_L_pos`, `_R_pos`, `Dummy_Hand_L_pos`, `_R_pos`). `parts(at,
  crashed)` is the plane plus the cone its state calls for on `PropDummy`; `pilot(at)` is where the
  pilot's root goes; `seat(set, lean, time)` poses a rider's body from `biplane_roll_fwd` /
  `biplane_roll_bck`; `emitters(text)` reads `biplaneparticles.dat`. One unit test (the emitter list).
- `--moment` draws the plane: a moment file with `vehicle biplane` (plus `livery`, `crashed`) draws
  the plane, its cone and the pilot instead of the bike. `main.rs`'s `load_biplane` opens
  `machine.res`, loads the plane, prints the seat it found and every emitter the plane would attach
  with "not drawn". The pilot is the shell's rider, posed by `seat` at the even mix of the two clips'
  first frames, its limbs held to the plane's anchors by the bike's own `hold_on` (all four hold,
  0.00 ft off).
- `research/shots/moment.py --biplane` writes such a moment out of a save state's RAM.
- Names and addresses for R8 in `research/oracle.txt` (section V21).

Where things are on the disc (read with throwaway examples, deleted):

- `machine.res` has everything drawn: `biplane\anim\biplane.psg` (and `biplane2`, `_ss` of both),
  `propconeblur.psg`, `propconebent.psg`, and under `biplane\tex\` the `.psm`+`.pst` of `pitts_body01/02`,
  `pitts_wing01/02`, `pitts_panel01`, `pitts_windshield01`, `guns`, `biplaneprop`, `biplanepropbroken`,
  and an `_ss` `.psm` of each. A freeride's `free0N_2.res` has the model, the cones, `biplane.phy`,
  `tracks\biplaneparticles.dat`, the five `.emt` and `appstart\biplanecpresets.ini` — but no texture.
- The model: 24 nodes, nose +z, five materials (body, wing, guns, panel, windshield). The cones carry
  their own spinner material: `biplaneprop` on the blur, `biplanepropbroken` on the bent one.
- `biplaneprop02` and `biplanepropbroken2` (named in the brief) exist only as lines in `startup.res`'s
  `appstart\mat_stbl.txt`, the material sort table. No archive has a `.psm` or `.pst` of either name
  and no model names them.
- Every rider archive carries `rider\anim\biplane_roll_bck.mot` and `_fwd.mot` (11 frames each).

Frames, all in `context/shots/` (not committed — game pictures):

- original: `v21-orig/{00,05,12,20,28,36,44,52}.png`, each the screenshot inside the save state of
  the same name, taken while playing `context/recordings/biplane.p2m2` on copy 4 (00 = the
  recording's own first frame, parked; 36 and 52 level flight; 12 a bank). `sheet.jpg` is all eight.
- port: `v21-port/{00,00-crashed,12,36,52}.png` from `v21-port/*.txt`, which are
  `moment.py --biplane` on the RAM of the same states:
  `./target/release/openmx --moment context/shots/v21-port/36.txt context/shots/v21-port/36.png`.

## What the original does

- The vehicle spawner (`./kit fn 0x0020eb10`) makes the biplane for kind 1 and picks the model by a
  livery index, 0–3 as above. The class is MXBiplane (`./kit fn 0x00353d10`) over a base Biplane.
- Load (`./kit fn 0x00354600` → `./kit fn 0x00350a70`): reads `biplane.phy`, loads the two cones as
  models of their own, writes (0, 3, -3, 1) to plane+0x900, and caches the four `Dummy_*_pos` nodes.
- Every frame (`./kit fn 0x00352ff0`): flaps, elevators and rudder turned by the control inputs; while
  plane+0x3b0 is zero, `PropDummy` is spun by the engine's prop angle, the blur cone is placed on it and
  shown and the bent one hidden; once it is set, the other way round. Show/hide is bit 6 of node byte 3
  (`./kit fn 0x001b3870`, `./kit fn 0x001b3850`). So the cone swap is a crashed flag, not a texture
  swap — which is why nothing ever needs `biplaneprop02`.
- The plane's own +0x180 is not its world transform (it reads as junk on every state); the world matrix
  is its model root node's (`*(plane+0x18c)+0x80`), in the bike's row convention.
- The pilot's root node (found by scanning RAM for transforms beside the plane) sits at exactly
  (0, 3.5, -3.0) in the plane's space with the plane's rotation, on states 00, 12, 36 and 52. That is
  `biplane::PILOT_SEAT`. With the clips' limbs about that root, the feet land 0.5 ft from the plane's
  foot anchors and the hands 0.2 ft from its hand anchors, so the hold closes a small gap, as on a bike.
- `biplaneparticles.dat`: the wreck's dust (`Biplanewreckdust.emt`) and chunks (`wreckchunks.emt`) on
  the collision model; `biplanesmoke.emt` on `Biplane_Exhaust_L`; `BiplaneExhaust.emt` on both exhaust
  nodes; `biplanetiredust.emt` on the three tire nodes; `Biplanedust.emt` (HelicopterDust) on
  `PropDummy`, fading in from 20 ft. Read by `./kit fn 0x00354750`.

## How it was proved

Looked right beside the original's screenshots of the same save states (render small, side by side):

- Level flight (36, 52) and the bank (12): plane in the same place on screen, the same size, the same
  attitude; red-and-white sunburst wings, guns, struts, wires, the pilot's helmet in the cockpit.
- Parked (00): the same plane from behind, nose up on its tail wheel, livery and cockpit right; the
  pilot's helmet in the seat.
- Crashed (00 with `crashed 1`): the bent blades on the nose instead of the blur. No original frame.

## Not verified / not 1:1

- **No emitters are drawn**: the port has no particle system at all (nothing reads an `.emt`).
  `BiplaneExhaust.emt` (both exhausts), `biplanesmoke.emt`, `Biplanedust.emt`, `biplanetiredust.emt`
  (three tires) and `Biplanewreckdust.emt` — plus `wreckchunks.emt`, which the list also attaches — are
  read from the disc's list and printed as "not drawn". The original's state 28 frame shows the dust
  under the plane; the port has none.
- **`biplaneprop02` / `biplanepropbroken2` cannot be swapped in**: neither texture nor material exists on
  the disc (only their lines in `mat_stbl.txt`). The crash swap the port does is the original's: the
  bent cone (`biplanepropbroken`) for the blur.
- **The prop cone is drawn cut-out, not blended**: the rasteriser has no alpha blending, so the blur
  disc (and the windshield) show only their opaque texels — dark blade blobs where the original has a
  faint red disc. The prop is also not spun (the original turns `PropDummy` by the engine's prop angle;
  that is S35's).
- **Flaps, elevators and rudder are at their bind pose**: the original turns them by the control
  inputs every frame; the port has no plane controls (S34/P19).
- **The pilot's pose is not the original's**: the even mix of `biplane_roll_fwd`/`_bck` at frame 0.
  Which clip the original plays and how far in depends on the plane's roll, which is S35's animation;
  only the seat itself was matched.
- **The 0.5 ft in the seat's height is measured, not traced**: the plane writes (0, 3, -3) at load
  and the pilot's root is at (0, 3.5, -3.0); where the extra half foot is added was not found.
- **The parked frame (00) draws the plane about 15% smaller than the original's screenshot**, while
  the three flight frames match in size. 00 is the first frame of the playback, just after the state
  loaded; the likeliest reason is that the camera in RAM at that moment is not the one the screenshot
  was drawn with, but this was not checked.
- **Only livery 0 was compared with the original**: livery 1 (`biplane2`) draws yellow (the
  `36-livery1` frame in `v21-port/`) but the recording only flies livery 0, so there is no original
  frame of it; the `_ss` liveries were not drawn at all.
- **There is no live biplane**: the shell cannot ride or fly the plane (P19 owns `--vehicle biplane`),
  so the plane is drawn only through `--moment`. The task's "`--shot` frames" are `--moment` frames.
- **The ground and sky differ from the original's** (sand-coloured ground, flat sky): the shell's
  track drawing, not this task's.
- The four states used are from one recording, all with the plane whole; no crashed state of the
  original was captured.

## Next

Nothing for V21. Particles (all six emitters), alpha blending for the prop disc and the windshield,
the spinning prop and the control surfaces wait on a particle system and on S34/S35.
