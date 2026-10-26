# 1 — track particles (FINAL)

Task V45. 2026-10-26.

## What I did

- **Split the task.** V45 named four systems: the `.toy`'s particle emitters, its sun flares, nat01's
  rain and weather box, and the toy sound loops. This entry does the emitters, which is the task's
  title. The rest are filed as follow-ups:
  - V71: the emitters that a trigger or the finish switches on (nat12's geysers, the sx fireworks);
  - V72: sun flares;
  - V73: nat01's weather;
  - V74: toy sounds.
- **`game_render::toy::emitters`** (`crates/game-render/src/toy.rs`) reads a `.toy`'s
  `[ParticleEmitterN]` the way the original's emitter toy reads them:
  - only `1..=NumberOfParticleEmitters`;
  - `ParticleType` → the class number, with the original's table and its fall-through to 0;
  - `StartOn`, on where it is missing;
  - `Position`;
  - whether any `AttachTo*` key is present;
  - `EmitterFilename`.
- **`particles::TrackFx`** (`crates/openmx/src/particles.rs`) runs the ones that build the plain
  emitter, stand at a `Position` and start on. Each one is a `Generic` entry of the same `PlaneFx`
  manager that P58/P74 built, on a node of its own: an identity turn at the emitter's position. It keeps
  the rest in `skipped`, each with a reason: starts off, on an object, or another class.
- **The shell** (`crates/openmx/src/main.rs`):
  - `load_track_fx` reads the track's `.toy`, the `.emt`s and the track's particle sheet;
  - the ride steps it on every simulated frame, beside the bikes' particles;
  - both renderers draw it with the plane's and the bikes' particles;
  - a world mod has none, because its tiles' `.toy`s are in tile feet, as V41 found for its cars.
- **What runs, per track**, from the disc:

  | Track | Runs | What |
  |---|---|---|
  | nat08 | 8 | 3 camp fires, 4 camp-fire smokes, 1 chimney |
  | nat09 | 2 | black smoke, bonfire |
  | nat11 | 3 | water mist |
  | nat12 | 2 | puddle fog, puddle mist; the 6 geyser emitters start off |
  | nat14 | 4 | 2 chimney smokes, 2 torch smokes |
  | nat15 | 1 | black smoke |
  | nat17 | 39 | 15 torches, 15 torch smokes, 9 water mists |
  | nat18 | 6 | chimney smoke |
  | sx04, sx20 | 13 each | water mist (`SX_WaterMist`, `WaterMist03/04`) beside their 30 fireworks |
  | free07 | 0 | |
  | the other sx tracks | 0 | fireworks only |

  The V41 audit missed sx04's and sx20's water mists. It also listed free07's Fire×7 and Smoke×20,
  but the original never reads them: see below.

## What the original does

Addresses are in `research/oracle.txt` § V45.

- **The list reader** reads `NumberOfParticleEmitters` and then `ParticleEmitter1..n`. free07's
  `[ParticleEmitters]` says `NumberOfParticleEmitters=0` above 27 full sections, so free07 has no fires
  or smokes in the original either.
- **The emitter toy's read** works through these keys:
  - `ParticleType`: the type names are compared with a case-insensitive strcmp, and anything else
    falls to class 0. The disc's `Generic` and `Smoke` both land there.
  - `StartOn`, `OrientWithToy`, `Zone`, `OffsetFromObject`, `RotateWithObject` and `VisibilityToyName`.
  - The attach chain. With no `AttachTo*` key, the emitter's point is the toy's own `Position`, from
    the base toy read.
  - Classes 0, 1, 2, 8 and 9 construct the plain emitter (`0x001503e0`). That is the class the plane's
    `Generic` entry constructs, which P58 ported and proved.
  - Then the emitter is initialised on no node at that point (set twice, so it has not moved), and
    switched on or off by `StartOn`.
- **The emit gate.** An emitter that was just switched on only records its point, so the first frame
  after load emits nothing. The port's `lit` latch does the same.
- **Every emitter that starts on** uses an `.emt` with `EmitMode=0` (continuous) and `DelayTime=0`.
  Each of those `.emt`s is in its track's first archive, and its group is in the track's sheet. So the
  port's existing emitter covers them without new keys. Every `EmitMode=1` (one-shot) `.emt` belongs
  to an emitter that starts off (V71).

## How it was proved

- **The disc.** `crates/openmx/tests/track_particles.rs`:
  - `every_tracks_emitters` reads 13 tracks' `.toy`s: free07, nat01, nat08, nat09, nat11, nat12,
    nat14, nat15, nat17, nat18, sx01, sx04 and sx20.
  - Per track it checks the count listed and that every emitter is class 0.
  - It checks which `.emt`s run and how many of each. These were tallied by hand from the dumped
    `.toy` files with awk.
  - It checks that every emitter left out is left out for starting off or riding an object, and never
    for a missing `.emt` or group.
- **The behaviour.** `nat08_smokes_where_its_fires_are`:
  - nothing in the air on the first frame;
  - every particle born on the second frame is within 2 ft of its emitter;
  - after 10 s, every emitter has particles in the air, and all of them are within 150 ft of a fire.
- **The reader** follows the decompilation listed above. Its emitter class is P58's, which was proved
  against the original's capture.
- **By eye.** `openmx --headless --track nat08 --trace-at 2378,2590,90 --shot` shows the camp fire
  burning in its stone ring by the trailer.
- **Gates.** `./kit check -- -p openmx --test track_particles` passes. `./kit audit` shows only
  DONE.md's leaks, which were already there.

## Not verified / not 1:1

- **Not compared against a live capture of the original.** No save state stands on a track with
  emitters: the motocross states are all free09, plus the biplane's free06, and neither `.toy` has an
  emitter. Getting the original onto nat08 means driving its menus by pad. The proof is the disc
  reader plus P58's proven emitter class. The particles' random draws are the port's own xorshift
  (seeded `0x5eed_0045`), as they are for the plane and the bike, so individual particles never match
  the original's.
- **View-dependent cull is not ported.** Every track `.emt` says `UseViewDependentCull=T`,
  `FarCullDistance=800`, `DirectionalCullDistance=200`. These are read into the emitter (+0x244,
  +0x24c, +0x250), but I did not find what reads them. The port emits from every emitter all the time
  and draws whatever is in front of the camera. If the original stops emitting from a far emitter, its
  fire would build up from nothing as you ride up to it, and the port's is already burning.
- **`Additive`, `Glows` and `ParticleColor` are not read.** The fires' `.emt`s say `Additive=T` and
  `Glows=T`. The port lays every particle over with the same alpha blend as the plane's smoke. This is
  rendering, matched by eye; the fire reads as fire in the shot.
- **World mods have no track emitters.** A tile's `.toy` is in tile feet and the emitters are not
  shifted, the same rule as V41's cars.
- **The other key reads are not ported.** `OrientWithToy`, `Zone`, `OffsetFromObject`,
  `RotateWithObject` and `VisibilityToyName` change nothing for a positioned emitter on the disc: none
  of them uses those keys. `Radius`, `Box`, `LookVector` and `UpVector` are base-toy keys that this
  emitter does not use for its point.
- **The emitters that start off or ride an object are filed, not done (V71).** These are nat12's six
  geyser emitters and the fireworks on every sx track. Sun flares (V72), nat01's weather (V73) and the
  toy sounds (V74) are filed the same way.
- **The shot used the shell's default renderer only.** The software renderer runs the same particle
  path (`particle::draw`), but I did not shoot it.
