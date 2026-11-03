# 1 — kiowa pilot seat (FINAL)

Task P216b1. 2026-11-03.

## What the original does

Read from the MX helicopter (0x007ccae0) in S148's and C14's RAM images (context/states, s3), and
from the ELF:

- kiowa_pilot_request (0x00391f10), the last step of kiowa_measure_mx, fills the request block at
  heli +0x550, which the ctor builds as the plane's is built:
  - base = the body's centre of mass and rotation;
  - offset pointer = &`PelvisBoxPosition` (+0xe60, (1.25, −2, 3));
  - phase target = 0.5·(1 − lateral);
  - mix target = 0.5·(1 + longitudinal);
  - band = +0x470, and the model's matrices.
- Unlike the plane's request, it **never writes** the holding flag (+0x55c) or the four limb targets
  (+0x600..+0x630). These keep the ctor's 0 and (0, 0, 0, 1), so the body plays its clips unheld.
  The model's Dummy nodes (`kiowa::SEAT`) are named but not used.
- The body step (0x0024934c) puts the root at base + rotation·offset. The model origin is the centre
  of mass less the turned `CofG` (0, 5.5, 0), so in model space the root is `CofG + PelvisBoxPosition`
  = (1.25, 3.5, 3). That is 1.25 ft right of the centreline, in the right-hand seat.
  - The plane's measured seat (0, 3.5, −3) is the same sum over biplane.phy. That explains the "half
    foot off" P216b found when carrying the plane's feet/hands offset across.
- 0x00390c70 writes (0, 3, −3, 1) to +0x7c0, as the plane does to +0x900. Nothing on the pilot's path
  reads it.
- In both images, a node sits at (1.2501, 3.5, 3.0002) in model space with exactly the helicopter's
  rotation. A run of 22 matrices 0x40 apart (the palette at 0x6271a0) lies within 3 mm of the port's
  unheld pose of the two clips at the request's (phase, mix) = (0.5, 0.5).
- The glass: kiowa_windows01.psm has three passes.
  - Pass 1 (0–95 ft): GlossySpecularMap, `alphaBlendingOnOff_1 = 0`, with an additive specular second
    stage (A0 B2 C1 D1, no z write).
  - Pass 2 (95–100): Diffuse, opaque.
  - Pass 3 (150–200): blended.
  - The .pst is 32×32 texels, all black with alpha 255.
  - So up close the glass is opaque black with a glint. The original never shows the pilot from
    outside, and the shell's black glass is right. The port's side and front shots
    (`--shot-pose` at yaw 90/180/270/135, 16 ft) show the windows black, with no pilot.

## What changed

- `game_render::kiowa::PILOT_SEAT` = (1.25, 3.5, 3.0), documented as the sum above.
- `SEAT`'s doc says the original does not hold the pilot to those nodes.
- `car.rs`: dropped the now-false "the same as the Kiowa's" clause from `SEAT_FROM_LIMBS`'s doc.
- `vehicle::PilotLean::step_heli(dt, heli)` steps the plane's lean with:
  - elevator = longitudinal and aileron = lateral, from the previous tick's pad, as the measure reads
    what the last input step left (C14's note);
  - held = false.
  - So neither a swap nor a put-back snaps the lean.
- `main.rs::heli_pilots` builds the palette with no limb holds. It dropped the `kiowa_drawn` and
  `assets` parameters, which only fed the holds.
- `tests/kiowa_picture.rs`: the seat moves to (1.25, 3.5, 3); the middle-of-pilot check reads x
  against it.
- New `tests/kiowa_pilot_seat.rs`:
  - kiowa.phy's and biplane.phy's `CofG + PelvisBox` equal the two `PILOT_SEAT`s;
  - on each Kiowa image (skipped where the slot has none):
    - the request's words: offset pointer, pelvis box, untouched limbs, held 0, phase/mix bits equal
      to `Pilot::axes()` on the image's pad;
    - the root node at the seat with the helicopter's rotation;
    - the 22-bone run where the unheld palette puts it.

Nothing was taken from opentennis.

## Checked

- `./kit check -- -p openmx --lib --test kiowa_picture --test kiowa_pilot_seat --test kiowa_switch
  --test biplane_pilot --test biplane_pilot_lean --test kiowa_particles --test bays --test menu`:
  all pass except menu's `original_on_sets_every_sub_switch_and_one_turned_back_leaves_the_rest`,
  which is red since P268 (the Fog choices) and already filed as P269. `kiowa_pilot_seat` ran both
  images in s3, not skipped.
- `./kit audit`: only the two plan/DONE.md leaks already in HEAD.

## Not verified / not 1:1

- Only grounded, level, centred-stick images were read: phase and mix are both 0.5 in S148 and C14.
  The formulas come from the disassembly and match bit for bit there, but no image with the stick
  over was compared.
- The lean's state on a swap into the helicopter (the ctor's starting phase and mix, and whether the
  body's filter starts at the target) was not read. The port starts it from `PilotLean::default()`.
- No PCSX2 picture of the original's Kiowa from the side or front: there is no pad here to move its
  camera. The "pilot not seen" verdict rests on the .psm and the .pst alone.
- The band (+0x6c8 = +0x470, the airspeed) is not passed to the port's body.
- The windows' pass 3 (150–200 ft blend) is not drawn by the port.
