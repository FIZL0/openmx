# 1 — biplane particles (FINAL)

Task P58. 2026-10-21.

## What I did

- `crates/openmx/src/particles.rs` (new): the original's particle system, as far as the plane uses it.
  - It reads `.emt` files over the original constructor's defaults.
  - It reads the track's `.prt` sheet (frames in pixels, divided by the `.pst`'s size) and its groups.
  - The emit rules, the per-particle update, and the vehicle manager's Generic, Exhaust and CollisionModel entries with their `Disable` flags.
  - It has its own xorshift RNG; the simulation's `rand` is never touched.
- `PlaneFx::new` takes `biplane::emitters` (the `.dat` list S39 already parsed). It skips the Dust and HelicopterDust entries and lists them in `skipped`.
- `crates/game-render/src/biplane.rs`:
  - `Emitter` gains `max_speed` (the Exhaust entries' `MaxSpeed`).
  - `Biplane::node` exposes a node's bind transform by name.
- `crates/game-render/src/gpu/sprites.rs` (new): camera-facing squares cut from one sheet.
  - Alpha-blended with no depth write, the `.psm`'s first pass.
  - Fogged like the models, drawn in every view after them.
  - `Renderer::set_sprite_sheet` / `set_sprites`. `gpu.rs` only gains the module, one field and one draw call.
- `crates/openmx/src/main.rs` wiring:
  - `load_plane_fx` reads the list, the sheet and the `.emt`s out of the track's archives.
  - `plane_fx_frame` builds each frame's nodes, throttle, airspeed and wreck from the ride's `Plane`.
  - The fx steps once per simulated tick, after the sim. Its sprites go to the GPU in both the offscreen and window paths.
  - The per-second plane line now also prints `particles: <entry> <count>, …`.
- `crates/openmx/tests/biplane_particles.rs`, nine tests:
  - Seven use hand-written `.dat`/`.emt`/`.prt` text: the list and skipped entries, the `.emt` defaults, the smoke's speed gate and carry, spreading along the path, turning with the node, the exhaust's throttle × MaxSpeed rule, the wreck dust off contacts, the update order and fades, and determinism.
  - The disc's own files: free06's 208-frame sheet; the five ported entries with their groups, nodes, gates and `Disable`s; the four skipped; then a flight through them.
- `research/oracle.txt`: a P58 block naming the sixteen functions read.
- Follow-ups filed:
  - P63: feed the wreck dust after S48.
  - P60: tire dust and prop wash.
  - P61: terrain collision and wind.
  - P62: the software renderer.

## What the original does

The plane's manager is built by `./kit fn 0x00354750` from `tracks\biplaneparticles.dat` (in each freeride `_2.res`). free06's list:

| entry | type | node | `.emt` | sheet group | disabled when |
|---|---|---|---|---|---|
| Vehicle Dust | CollisionModel | – | biplanewreckdust | Dust 1 (49–64) | NoContact, ObjectContact |
| Vehicle Chunks | CollisionModel | – | wreckchunks | Chunks 1 (17–32) | NoContact, ObjectContact |
| Smoke | Generic | Biplane_Exhaust_L | biplanesmoke | Smoke 2 (126–141) | Wrecking |
| Left/Right Exhaust | Exhaust, MaxSpeed 35 | Biplane_Exhaust_L/R | biplaneexhaust | Smoke 1 (159–174) | Wrecking |
| 3× tire dust | Dust | tire particle nodes | biplanetiredust | – | – (not ported) |
| Dust | HelicopterDust | PropDummy | biplanedust | – | – (not ported) |

- **The manager's frame** (`./kit fn 0x003d5aa0`):
  - It sets Wrecking (8) or NotWrecking (0x10), plus NoContact (1), GroundContact (2) or ObjectContact (4).
  - An entry runs unless `Disable & flags`. An entry switched off forgets its last point.
- **Generic** (`./kit fn 0x003d6e60`): one emitter on its node.
- **Exhaust** (`./kit fn 0x003d5f70`):
  - `f = throttle × ½ × (MaxSpeed − airspeed) / MaxSpeed`. Nothing when `f ≤ 0`.
  - The file's velocity.y is multiplied by `f + ½`.
- **CollisionModel** (`./kit fn 0x003d75d0`): one emitter per body contact, with its previous point `contact − body velocity × dt`.
- **The emitter** (`./kit fn 0x0014daa0`, `0x00151d80`, `0x00150540`, `0x00152030`):
  - Its speed is `|pos − prev| / dt`. Under `MinimumEmissionSpeed` it emits nothing (smoke 35 ft/s, wreck dust 3).
  - Otherwise `carry += rate × dt`, and it emits `floor(carry)` (at least `MinParticlesPerFrame`), spaced along prev → pos ending at pos.
  - Velocity is the file's turned by the node, plus noise drawn from a ball, plus the emitter's own velocity if `ApplyEmitterVelocity` (the wreck dust only).
  - Life, size and growth are drawn between the file's two values.
- **The update** (`./kit fn 0x001556b0`):
  - `age += dt × 1000` ms; dead once past life.
  - Grows: `size += growth dt`, and the centre rises by the same.
  - Moves: `pos += vel dt`.
  - Drag: `vel −= vel × drag × dt`.
  - Gravity: `vel.y −= mult × 32.2 × dt`.
  - Then terrain collision and wind (not ported).
- **The draw** is VU1 microcode (`Vu1Particles.vuo`), named by the sheet's `.psm`. Pass 1 blends `(Cs − Cd) × As + Cd` with Z writes off.

## How it was proved

- **Function-level rules:** the seven synthetic tests in `crates/openmx/tests/biplane_particles.rs`, each checked against the decompilation above. All pass under `./kit check`.
- **The disc's data:** `the_discs_biplane_particles` reads free06's files and checks every entry's kind, node, group, gate and disable flags.
  - A temporary panic at its end proved it runs here rather than skipping: it reached the end with `Smoke 52, Vehicle Dust 1, Vehicle Chunks 4`. The panic was removed.
- **Live:** `openmx --vehicle biplane --play <tape>` with Cross held, then the stick, looped the plane over free06. The per-second line showed:
  - exhaust only on the takeoff roll under 35 ft/s (17–22 a side, 0.2–0.4 ft puffs at the cowl);
  - smoke from 35 ft/s (36.7 mph) on, about 100 up at a time;
  - no exhaust once past MaxSpeed.
- **By eye:** shots at 11 s (takeoff) and 18 s (dive) beside S57's capture video at 32.5–34 s:
  - The original's dive puffs are a soft white haze over the cowl; the port's look the same.
  - Without the ×2 alpha below they were invisible: the sheet's smoke texels are at most about a quarter opaque. A debug shader showing the alpha confirmed the puff shape is in the texture.

## Not verified / not 1:1

- **The draw is matched by eye**, because Vu1Particles.vuo cannot be read. Each choice:
  - Sprite size taken as the full width in feet.
  - Alpha ×2: assumed from the GS letting vertex alpha reach 0xff; picked against the capture.
  - The fades are linear.
  - The colour is the texel's, unlit, with the models' fog.
  - Squares are not rotated.
- **No frame-by-frame comparison** with a PINE capture of particle counts or positions; only the per-second counts and two shots beside the video.
- **The wreck's dust and chunks never draw live.** The port's plane has no body contacts until S48, and its only crash is the map edge, so `plane_fx_frame` passes none. They are tested on synthetic contacts and the disc's files only (P63).
- **Wind and terrain collision/restitution are not in the update**, so chunks would fall through the ground (P61).
- **Tire dust and the prop's HelicopterDust are not ported** (P60).
- **Node transforms are the model's bind pose times the plane's transform.** Any animation of the exhaust nodes (none expected: they hang off the fuselage) is ignored.
- **The software renderer draws no particles** (P62).
- **Atlas bleed:** the sheet has no mips and the sampler is linear, so a frame's edge may bleed a texel of its neighbour. Not seen in the shots.
- **Timing is per simulated tick:** the fx steps with the sim's dt (30 Hz), as the original's manager does with its frame. It was not checked against the original at other frame rates.

## Next

Nothing in P58. After S48, P63 feeds the contacts and checks the wreck puff against S57's video.
