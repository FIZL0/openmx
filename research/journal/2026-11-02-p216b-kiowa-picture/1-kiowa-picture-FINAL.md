# P216b: the flown Kiowa's picture and camera

## What changed

- **Slide, cut, seam.** `heli_at`/`heli_rotor`/`heli_drawn` (main.rs, beside `plane_at`): the Kiowa
  is drawn between the tick before (`before_helis`, kept as `before_planes` is) and the tick at the
  frame's blend, its rotor angles too (`Rotor::between`). The tick it is taken is a cut.
  `cut_spots` and `seat_seam` read the helicopter's place when one is up, and the seam carry moves
  its `before` with the world. `heli_jumped` (the measure's speed against how far it moved) makes
  the follow camera `follow_jump` as `plane_jumped` does.
- **Pilot.** `heli_pilots` puts the rider's own body in the seat, as V39's plane pilot: root
  `game_render::kiowa::PILOT_SEAT`, its hands and feet on the Kiowa's seat nodes, leaning with
  `PilotLean::step_heli` on the cyclic (the original loads the biplane_roll clips for vehicle kinds
  1 and 2, so the plane's lean clips are the helicopter's too). Both renderers draw it and trace it
  under `--trace-picture`.
- **Software renderer.** `kiowa_draw::skinned` skins the Kiowa's pose into a per-seat copy
  (`kiowa@seatN`), drawn with its pilot on the software path.
- **Camera.** `Rig::heli_camera` gets the seat's presets and the cameras page's framing
  (`frame_heli` at the P82 reframe, `set_heli_display` after every `set_display`), the per-seat mod
  eye ceiling (P136's `plane_eye_ceiling`, also on a modifier change), and P163's start: the preset
  cycled to is kept (`camera_left_on(.., Machine::Helicopter, ..)` → `Seat::heli_start`), written as
  a third start on the `camera_start` line only when set, so older files read and write as before.
  The Camera page lists the disc's helicopter presets after the plane's (`heli_camera_names`, off
  free06_2.res's helocpresets.ini).
- HEAD did not build (P216a made `Heli::touching` a method, V114 read the field): fixed both reads.

## The seat

`[Animation] PelvisBoxPosition` is not the seat: the plane's is (0, 0, -0.5) against its seat root
(0, 3.5, -3); the Kiowa's (1.25, -2, 3) with size 0 put the body 4.7 ft off. `PILOT_SEAT` is the
plane's root offset from the mean of its feet/hands seat nodes, carried to the Kiowa's
(-0.107, 3.522, 0.937). Drawn, the body's middle is ~0.5 ft over and in front of the root, between
the pedals and the stick.

## Checked

- `crates/openmx/tests/kiowa_picture.rs` (new, ran — no skip, --nocapture): free09, Cross held,
  switched in at tick 10; on gpu and software every helicopter frame after the cut is drawn at
  `from + (to - from) * blend`, some between two different heights; each has a pilot rooted at
  `PILOT_SEAT` with its body within 3 ft.
- `tests/menu.rs::the_helicopter_camera_is_kept_and_framed_per_seat` (new): kept start, the
  settings line and its round trip, an old two-start line, `start_on` on the kept name, the page's
  labels, an own preset laid on the helicopter's button only.
- `./kit check -- -p openmx --lib --test kiowa_switch --test kiowa_picture --test menu --test
  biplane_pilot --test kiowa_particles --test overlay --test bays`: 348 passed; `--test
  machine_packs --test camera_view --test thrill_camera --test markers --test smooth_picture
  --test wreck_camera --test ride`: 392 passed.
- By eye: `--shot` frames behind, beside and in front of the Kiowa on gpu and software: the same
  helicopter on both, rotor blur, dust.

## Not verified

- The seat root and the lean's inputs are derived/guessed, not read off a Kiowa state (P216b1).
- The pilot is not seen in any shot: kiowa_windows01 is opaque (`blend` off) in its .psm, so the
  glass is black. Whether the original shows the pilot through it is P216b1's question.
- Split screen and online with the helicopter were not ridden; the per-seat paths are the plane's.
- No side-by-side with the original's picture or camera.
