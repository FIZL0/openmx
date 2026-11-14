# 1 — atv camera sound (FINAL)

Task MD136. 2026-11-14.

## What I did

Gave MX vs. ATV's quad (MD37d) its own follow camera, engine note and tire particles, all read again
off the machine pack it rides off (`atv_own` in `crates/openmx/src/main.rs`, run once per ride after
the garage is built when `--atv` picked a quad).

- **Camera.** `chase::atv_presets` reads `Cam\FC_ATVDefault.pb` through the fliers' reader
  (`chase::flier_presets`, the MX vs. ATV `[4x3/1p/n]` sections), falling back to `Cam\FC_ATV.ini`
  only where the `.pb` has no presets. The picked file's bytes go into `car_presets` as
  `(Kind::Atv, bytes)` and a `Race` camera into `car_cameras`. The rig already looks a machine's camera
  up there by `garage.up`, so the quad is followed on it with no other change. `kind_presets` sends
  `Kind::Atv` to `flier_layout_presets` (the screen shape and split as the fliers map them) and the
  cars to `layout_presets`. It is used when the camera is built and in `cars_for_display`, so a
  display or split change re-reads the quad's for that screen. The camera starts on preset 0
  ("Race Camera 1"); the cars' `Race Cam 1` lookup is skipped for it. The Camera page lays its
  own `Quad` presets over it through `frame_cars` (`menu::Machine::from_word("atv")`), as for the cars.
- **Engine.** `Pack::engine_sounds` (the bike's `.aud` reader, the same ten sections) reads the
  pack's `Aud/vehicle.aud`: 22 sounds and 4 wreck sounds in the Race 250 pack. `audio::SeatFrame`
  gained `atv: Option<Hands>`, filled by `atv_hands` while the quad is up and out: on the
  ground, the throttle after its rate limit, the clutch lever, the body's speed, where the body is,
  and the wrecked flag. While it is set, `Sound::frame` stops the bike's engine, edge rumble,
  drive-on and flier voices, and plays the seat's `atv_engine` (an `audio::Engine` of its own) on
  the pack's lists, set by `Sound::set_own_atv`, or on the bike's where the pack has none.
- **Particles.** `load_atv_fx` builds a `particles::PlaneFx` off the pack's `Part/atvparticles.dat`,
  with each `.emt` out of the pack, else the track's. The sheet is the track's. The zone table is
  the pack's `Part/atvparticlezones.dat` (new `particles.zones` word kept by `machine_packs::Pack`).
  The fliers (MD108) keep the track's, but the disc's names no descriptor at all. Under it the
  quad's `WaterRoost` entries sprayed water off free09's dirt from the first tick (seen:
  `Rear Left Tire WaterRoost 4` at tick 60). The pack's table turns `WaterRoost` off everywhere but
  zones 7–8. `atv_fx_frame` places the emitters' nodes where the chassis model hangs under the
  quad's scene node (`quad::Model::worlds`) and feeds the four wheels in `AttachedToTire` order
  (LF, RF, LR, RR, the quad's own wheel order): ground and object contact, compression, the surface
  record byte, spin and side force per load. It also feeds throttle, speed, clutch, wreck and the
  body's velocity. One manager per seat is stepped while the quad is up and off otherwise. It is
  drawn in the GPU and software paths after the cars', and the end of the ride says what is in the
  air.

Lines the shell now says (free09, `--atv atv_Race250P`, 130 ticks):

```
atv: mxvatv_atv's camera, 8 presets in Cam\FC_ATVDefault.pb: Race Camera 1, Race Camera 2, Offset Camera Right, Slot Camera Right, Offset Camera Left, Slot Camera Left, Look Back Camera, 1P Stunt Camera
atv: mxvatv_atv's engine, 22 sounds in Aud/vehicle.aud, 4 wreck
atv: its camera starts on Race Camera 1, 8 presets in Cam\FC_ATVDefault.pb
atv: particles: zones off Part/atvparticlezones.dat
atv: particles: Right Exhuast rides 250_Exhaust_Particle01, which the chassis model does not have
atv: particles: 18 entries off mxvatv_atv's Part/atvparticles.dat; 2 tire tracks not laid; 4 not run: Front Left Tire Water Splash (Water), ...
atv: atv_Race250P 139.8 ft from where it was taken, ... 88.0 ft/s, on all four, wrecked 0 times
atv: particles: Right Tire Dust 18, Left Tire Dust 17 in the air
```

A temporary probe (removed) put the rear particle nodes about 1.7 ft to each side of the chassis
origin and 2 ft behind it along the direction of travel, which is about where the rear tires are.

## What the original does

Which camera file MX vs. ATV's follow camera loads for the ATV was already read off RAM by MD82a:
the follow camera object's file pointer names `Cam\FC_ATVDefault.pb` in `md37a/atv-r-spawn` and
`atv-s-air` (crates/openmx/tests/mxvatv_camera_presets.rs). Its sound and particle update code was
not read for this task.

## How it was proved

- `crates/openmx/tests/atv_camera_sound.rs`:
  - `the_quads_presets_are_fc_atvs` reads the pack's files and checks FC_ATVDefault.pb's eight
    preset names in order, for four screen shapes, plus the `.ini` fallback and the none case.
  - `the_quad_rides_its_own_camera_engine_and_particles` rides the binary headless (free09,
    130 ticks, a switch to the quad at 30) and checks the camera lines (names, file, start preset),
    the engine line (sounds > 0 in `Aud/vehicle.aud`), the zones and list lines, and tire dust in
    the air at the end.
- `mxvatv_atv.rs` (MD37d) still passes. Its bike-only ride hash is unchanged with a quad picked.

No PCSX2 comparison was made.

## Not verified / not 1:1

- **The engine note is MX Unleashed's, not MX vs. ATV's.** The quad plays the bike's engine voice
  (`audio::Engine`: levers, ground and speed choose the idle, low, high, power, decel, clutch,
  sputter and panic loops and pitch them) over the pack's `vehicle.aud` lists. The task says "on
  its rpm". MX vs. ATV's own sound update was not read, so whether it pitches off the quad's
  engine rpm (`ride.bike.engine.rpm`) and how is unknown. Feeding rpm into a voice whose pitch
  curves were fitted to MX Unleashed's speed and levers would be a guess of its own.
- **Not heard.** Headless runs have no audio device. The test checks that the sounds are read and
  that the frame carries the quad's hands, not what comes out. Not listened to beside the original.
- **The `[Wreck]` clank is not played for the quad.** The bike's wreck sounds hang off the
  rider's wreck emitters (P202), which the quad does not drive. The 4 wreck sounds are read and
  counted only.
- **Zone numbers.** The pack's zone table is keyed by MX vs. ATV's surface numbers. The quad's
  wheels report MX Unleashed's surface byte off the track ridden. Whether zones 7–8 (water in the
  ATV table) mean the same surfaces on MX Unleashed's tracks was not checked.
- **The exhaust is off.** `Right Exhuast` rides `250_Exhaust_Particle01`, which neither chassis
  `.psg` has. Its node is left unplaced, so it emits nothing.
- **Water splash is not run.** The four `Water` type entries are a type the port's manager does
  not have (the bike's lists have none). The two `TireTrack` entries are not laid: the quad has no
  tire tracks yet.
- **No collision contacts fed.** `CollisionModel` and constraint entries get no contacts: the
  quad's collision record is not put into the frame, so its collision chunks come only from
  per-tire `ObjectContact`, not from a body hit.
- **Tire words assumed.** Compression is the wheel's `compression` as the quad's wheel keeps it,
  in its own units, not checked against what the `.emt` ramps expect. `spinning` and `side` are
  the tire's slip flag and side force per load. Which of the quad's words the original's tire
  records point at was not read.
- **Particles not seen.** The quad is not drawn yet (MD135), and no shot was taken. The particles'
  look and their place on the wheels were checked only through the probe numbers above.
- **The ATV's AI list and the rear camera are unused.** `Part/atvaiparticles.dat` (AI riders) and
  `Cam\RC_ATVDefault.pb` are not read: there are no AI quads, and no rear-camera mode exists.
- **Online.** The quad is offline only (MD137), so none of this runs online.

## Next

MD160 (filed): MX vs. ATV's own engine sound update (whether and how it reads the quad's
rpm) is the open question above.
