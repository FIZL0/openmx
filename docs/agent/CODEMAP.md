# Code map

Read only the section for the area a task touches (`./kit plan <section>`, e.g. `./kit plan game-sim`), not the
whole file. Keep it current: it exists so nobody greps the repo to find where something lives. As you add
modules, list each under its crate with one line, the way `ps2kit/` is listed. A sentence per file is worth
more than a directory listing, and it is the cheapest context in the project.

## Layout

`ps2kit/` the tooling · `watch/` capture specs (`*.toml`), one per thing you record · `research/` your own
scripts and notes, committed, no game data · `context/` everything game-derived, git-ignored · `crates/` the
port (yours to shape): the kit's `ps2-float`, `ps2-oracle`, `game-sim`, plus `game-render` and `openmx`.

## ps2kit

The kit's tooling, all of it generic; `docs/INTERNALS.md` has the contract.

- `config.py` reads `kit.toml` (the only game-specific file) · `platform_.py` the whole OS surface
  (the only OS-specific file) · `cli.py` dispatch
- static: `disc.py` ISO 9660 · `elf.py` PS2 ELFs and overlay ELFs · `ghidra.py` headless analysis ·
  `decomp.py` read the dumps one function at a time
- live: `pine.py` PCSX2 memory · `pad.py` virtual controller · `emu.py` launch/stop/screenshot/states ·
  `probe.py` find addresses · `record.py` declarative captures
- workflow: `plan.py` · `journal.py` · `checks.py` · `runner.py` + `runner.toml` the kit's settings for
  [agent-runner](https://github.com/FIZL0/agent-runner), which runs `./kit plan` and `./kit run` · `oracle.py` ·
  `setup.py` setup and doctor · `hooks/` the context guards

## ps2-float

The PS2's arithmetic: EE FPU add/sub/mul/div/sqrt/madd, VU0 macro ops, the game's own transcendentals. Everything in the simulation goes through this.

## ps2-oracle

A small R5900 interpreter that runs one function from the game's own code so a test can compare your port against it over thousands of inputs.

## game-sim

Where your port goes. Starts nearly empty, with the replay harness and the state hash, because those two are what keep everything after this honest.

- `hash.rs` — the state hash: one walk over the whole state, folded into 64 bits for a per-tick
  comparison or dumped field by field to name the first field that differs (A1). Every state type
  describes itself once, with `impl hash::State`.
- `input.rs` — the input layer: the pad read once a frame before the simulation, mapped into the
  game's own bitmask and kept as sixteen controls a pad — held, strength, the press and release
  timers — plus the one-frame event queue the front end reacts to. The original keeps no "pressed
  this frame" mask, so the edge is the event and "held with the timer at 0" (I1). The same poll's
  six analogue axes: the report byte each one reads, the inversion one of them gets, and the dead
  zone of ±0.2 that is the only shaping there is — no clamp and no curve (I2).
- `markers.rs` — session markers (P88), the port's own: five slots a player, a d-pad direction's
  press (P93) to step the selection (slots, then the other players), place the bike as it is, or
  teleport back ghosted; in the shell the d-pad leaves the steering to the stick. Pad input on agreed ticks, state in `Sim`, hashed and sent only once used.
- `modifiers.rs` — the game modifiers (P55): twelve percentages, per player or for the world, that
  scale fields of the loaded `.phy`, the built ride's speed limit, the biplane's file and the
  engine's time scale before a ride starts — never a branch in a step, and nothing touched at 100%.
  Their text form is what `settings.txt`, a tape's `modifiers` line and a lobby's room carry.
- `replay.rs` — the replay harness: a fixture of inputs and expectations replayed tick by tick (A2),
  plus the pad tape — a recorded session as the report bytes and measured `dt` of each frame, which
  `Playback` plays back through the input layer so a human-played sequence drives the port (I3), and
  the reader for a human's PCSX2 input recording (`.p2m2` → `PadTape`, I4), and the two ends a live
  loop rides one with: `Recorder`, which writes a tape down a simulation pass at a time, and `Head`,
  the playback head it is read back through (P7).
- `rng.rs` — the C library's 64-bit LCG the whole game calls, plus the random float and the uniform
  list index its call sites wrap it in. Proved bit-exact in `tests/rng.rs` (R5).
- `track.rs` — one track as the disc stores it: the `.scn` scene (spawn, terrain layout, every model and
  collision placement), the `.col` shapes the simulation tests against in their own bounding-volume tree,
  and the `.trn` quantised heightfield with the game's own height and surface query. Proved against
  free06 in `tests/track.rs` (D6).
- `tick.rs` — the fixed update: one frame as the original runs it. The mode mailbox, the Timer 0
  clock, the thirty-entry `dt` average with its 30 fps clamp, the pad latch, the markers around the
  simulation, and the three different ways a frame can not simulate. `Engine::step` is the top-level
  step everything else hangs off. Proved against `engine_tick` itself in `tests/tick.rs` (S1).
- `actor.rs` — actor lifetime: the two intrusive lists a level's live objects hang in, the interned
  name an actor's "kind" actually is, the three steps of creating, registering and freeing one, and
  the master list's order — which is registration order, and so is the order the per-frame dispatches
  and the two "nearest actor" searches resolve in. Proved against the manager, the record's
  constructor and both destructors in `tests/actor.rs` (S2).
- `air.rs` — the bike in the air: where the sim decides the wheels left the ground, the hang time and
  the crossup angle, the preload and the backflip taken off a lip, what the stick still commands while
  airborne, and the test a landing has to pass for the rider to stay on. Carries the game's own
  double-precision arc cosine and the startup table built from it. Proved against the original over
  20,000 generated substeps in `tests/air.rs` (S10).
- `trick.rs` — tricks: the 57-stunt table the disc ships and each player's 24-slot gesture map, the
  stick-and-button gesture that names one, the hold/safe/duration clocks that decide whether a trick
  counts, and the trick's own half of the landing test. Includes the engine's shared arc-cosine
  table, which the stick classifier reads. Proved against the disc and the oracle in
  `tests/trick.rs` (S11).
- `collide.rs` — collision against the world: the triangle test the original sweeps a bundle of probe
  segments through a collision mesh's tree with, the box rejections that keep a probe out of a branch,
  how a hit resolves into the blocked travel a caller takes off the motion, and the two limits the
  sweep allows itself (16 probes, 256 levels). Proved against the original's sweep over 10,000
  generated scenes in `tests/collide.rs` (S4).
- `state.rs` — the main actor's state machine: riding, down after a wreck, waiting, put back. The
  reasons a rider comes off, the settle window and the wait that decide when it goes back on, the pad
  chord beside them, which transitions share a frame, and the fact that a down rider's physics does
  not step at all. Proved against the original machine, its entry and its put-back in
  `tests/state.rs` (S5).
- `move.rs` — movement on the ground: the per-frame step that turns the throttle, the brake and the
  rider's lean into forces on the chassis. Where the mass acts and how far the stick moves it, the
  steering lock fading out with speed, the donut's roll, the suspension rates, the gearbox and the
  clutch's launch ramp, the drag that sets the top speed, the rev limiter — and the substep loop
  that is the order the whole ground frame runs in. Plus the engine behind all of it: the power
  curve, the clutch's slip and lock-up, and the torque that reaches the driven wheel (S15). The
  twenty callees it still drives (suspension, tires, roll and pitch) are not ported yet, so the step
  returns the ordered list of them instead, which is what makes the order provable. Proved against
  the original over 2000 generated frames and a 240-frame ride in `tests/move.rs` (S3, S15).
- `chassis.rs` — the rigid body under the bike: the integrate virtual the ground step ends every
  substep with (the transform's own turn carried onto the body, force and torque through the inverse
  mass and inverse inertia, four two-step integrations, the speed and per-axis limits, the six
  freedom flags, the contact correction and the transform written back), the per-frame measure that
  gives the body its speed, the transform's rotate/unrotate pair, the two-step integrator itself and
  the contact distributor that spreads a frame's collisions onto the two wheels. The transform's own
  arithmetic is not ported, so the step returns the ordered list of what it called. Proved against
  the original over 10,000 generated bodies each in `tests/chassis.rs` (S19).
- `phy.rs` — the bike's `.phy` file as the port reads it: the game's own plain-text reader
  (whitespace removed rather than trimmed, `strtod` and then the truncating conversion to a float,
  three delimiter sets for a quadword, `T/F/ON/OFF/Y/N` before `strtol`) and the whole of
  `bike_phy_load` on top of it — seventeen sections into the bike, its chassis, its two wheels, its
  engine and its collision record, filling `move.rs`'s params and `air::Params` together. Proved
  against the original four ways in `tests/phy.rs` (S18): every key of three files read six ways,
  960 words of the bike from a zeroed start, all 128 setter calls' arguments, and the bike in the
  human's own recording. The four rider controllers' twenty-five gains, limits and speed bands
  reach `move::Params` as copies of the loader's own words, checked bit for bit on all eighteen
  `.phy` files the disc carries (S21).
- `camera.rs` — the camera for normal play: the 160-byte preset record the disc ships one of per
  camera, the target point and the look-ahead it leads the rider by, the eight first-order lags that
  make it follow instead of snap, and the limits on its yaw, pitch, distance and field of view.
  Presentation, not simulation — it reads the rider and writes a view, and touches neither the RNG
  nor the clock. Proved against the original in `tests/camera.rs` (S7). Its two questions about the
  world are `trait Surface`, the ground under the look-at point, and `trait Obstacles`, the nearest
  solid thing in front of the lens; `Nothing` answers neither, which is what the class the original
  reads answers, and a track answers both through `world.rs` (S23). `Follow` is the base class, and
  the disc never instantiates it on its own: `Race` beside it is the derived class the game creates
  for a race, a near-copy of the same frame with its own aim, look-at and position and no obstacle
  push-out, and it is the one the shell runs and the one later camera work is measured against
  (S24).
- `motion.rs` — animation driving: which motion a rider's state plays, how far into it the frame is,
  and how two of them blend. The action-id switch, the cross-fade that makes a motion start from the
  pose the rider already had, the airborne pair chosen by the fore-aft lean with the shown crossup a
  landing is judged on, the clip clock with its loop and its end, and the window one frame opens for
  the action frames inside it. Proved against the original's driving, advance and window over 10,000
  generated frames each in `tests/motion.rs` (S6).
- `vehicle_anim.rs` — the caller `motion.rs` was written for: the vehicle's own animation state
  machine, one frame of it. Where the vehicle puts itself each frame, the sixty-entry switch on its
  animation state and the fade rate each state restarts with, its own clip table loaded off the
  stunt names, the two lags that carry the pose grid's axes, the extra records a track can add past
  the switch, and the arm that crosses a gesture out over the pose it was played from. Picks the
  blend `motion.rs` runs — including the four-clip blend and the sixth entry point that nothing else
  in the image reaches. Proved against the original one state at a time over 24,000 generated frames
  in `tests/vehicle_anim.rs` (S25).
- `rider_anim.rs` — the half in front of `vehicle_anim.rs`'s step: the **rider's own** animation
  selection, which is what decides the state the step is handed. The gate's get-ready sequence (the
  breathe, the ten no-repeat idles and the three start poses), the wreck's flail and its five held
  limbs, the gesture and the no-hander that replaces it, the backflip, the airborne pose, the
  reverse pair and the look-around that fires while the bike stands still — the clips of a rider's
  own set that nothing the bike asks for could ever reach. Its answers to S25's two open questions:
  the actor that step belongs to is the rider's *body*, and this is the writer of both fields the
  step only reads. Proved against the original arm by arm over 30,000 generated frames in
  `tests/rider_anim.rs`, with the gate and the wreck also driven through a whole `rider.rs` (S27).
- `world.rs` — what the simulation asks the world: the ground under a point (height, normal and
  surface, from the ported terrain query) and the scene's collision placements swept as one bundle,
  each placement's mesh in its own space, so a prop stops the rider the way the original's does.
  `trait World` is the one seam between the ride and a track; `Flat` is the void a test rides in
  (P2). The follow camera's two probes are answered here too, off the same terrain and the same
  placements (S23).
- `ride.rs` — **the bike as one state**: the player's ride, driven by `move.rs`'s ground step with
  `wheel.rs`, `chassis.rs`, `air.rs` and `collide.rs` behind it on the parameters `phy.rs` reads off
  the disc. The input layer's rate limit, the frame's four steps in the original's order, and the
  answers to every callee those modules hand out — the suspension ray, the tire patch, the wheel's
  spin and the body's sweep. `impl hash::State` walks every word of it, which is what makes a
  freeride replay comparable. The ride that `openmx` rides (P2).
- `rider.rs` — **the person on the bike**: `trick.rs`'s machine, `air.rs`'s wreck test and
  `state.rs`'s ride states wired onto `ride.rs` in the order the original's ridden actor runs them —
  the latch, the physics, the gesture, the request, the clocks and the animation header, then the
  machine carrying the wreck test. Joining is all it is: every number comes off the disc and every
  decision belongs to one of the four modules it calls. `Rider::body_frame` is the second entry
  point, for the rider's own animation: the body is an actor of its own, and this is where the
  request block `rider_anim.rs` reads is published off the rider's own fields (S27). The rider that
  `openmx` rides (P3).

## game-render

The drawing side: the disc's own models and textures, on screen. Host `f32`, no bit-exactness, nothing the simulation depends on.

- `disc.rs` ISO 9660 → `MX.PSR` → a `.res` member, read from the user's disc at runtime ·
  `psg.rs` `.psg` geometry: nodes, materials, LOD groups, VIF streams → triangle strips ·
  `pst.rs` `.pst` textures: a 256-colour palette in the GS's order, plus its mip chain ·
  `terrain.rs` `.trn` ground: a patched height field, its palette and its ground texture
- `ini.rs` the plain-text format the disc's data files share · `scene.rs` `.scn` scenes, `.psm`
  materials, `.sub` bike builds and the `cpresets.ini` follow cameras · `world.rs` a track in
  memory: models, materials and textures matched by name, and drawn (a scene prop by its placement
  alone, not its file's root turn) · `toy.rs` the `.toy`'s path cars — free06's train, its trucks
  and scrapers, the trailers — on their `.mot` clips at the original's 1/30 s scene step, and its `[ParticleEmitterN]` list, which `openmx`'s `particles::TrackFx` runs (V45)
- `sun_flare.rs` the `.toy`'s `[SunFlareN]` read as the original's reader leaves them, and each drawn as an additive glow at the sun's screen point, the ground hiding it (V72)
- `shadow.rs` the Shadows setting (Better, Original, Off), the original's tint and ranges, and the sun's boxes for shadow maps · `gpu/shadows.rs` drawing them: two cascades along each view's look for ours, a small map round each racer for the original's, read by the mesh and ground shaders (V108)
- `raster.rs` a depth-buffered, perspective-correct, textured triangle rasteriser · `png.rs` a PNG
  writer · `mat.rs` vectors and 4x4 transforms in the files' own convention ·
  `bin/first-draw.rs` the whole chain: disc in, one frame of a track out
- `rider.rs` the rider: its `.sub`, the `.mcf` it names, its suit and how it bolts onto the chassis ·
  `chassis.rs` where the renderer put the bike, against where the solver put it: the one placement
  builder the shell and the tests share, the live read of the chassis' four grip anchors, and the
  per-wheel measurement of the drawn tire against the contact the suspension found (V20) ·
  `skin.rs` `.mot` motion clips and the one `Skeleton` a model is read into: the channels, the
  quaternion blend, the joint map a clip is bound through, and the pose on a model (V4, V5)
- `anim.rs` — **the animation seam**: the port's one implementation of `game_sim::motion::Animations`.
  `Set` is one animation set — a `.mcf`, the `.psg` it names as its skeleton, and the clips of that
  set read off the disc and *bound* to it, which is where the joint map is built and kept; `Rig` is
  that set with the simulation's driver, clip table and clock beside it, so one call is one frame:
  the selection, the clock, the sample, the pose, the bones. The simulation names a clip and knows
  nothing about where it comes from; this is the only thing in the chain that opens a file (V7).
- `biplane.rs` the biplane: one of its four liveries and both prop cones out of `machine.res`, the
  cone its crashed flag calls for on `PropDummy`, the pilot's seat and its two roll clips, and the
  emitter list it would attach (none drawn: there are no particles yet). Drawn through `--moment`
  with `vehicle biplane` (V21)

## openmx

The playable shell: the binary a person rides. A window, a real pad, the ported simulation at the game's own 30 Hz, and the freeride area drawn with the renderer, with no emulator, no fixture and no menu in the way (P1).

- `main.rs` — the loop: the disc's track and bike, the scene read twice (the simulation's reader for
  the spawn, the terrain and the collision placements, the renderer's for props, lights and fog),
  then a frame of `tick::Engine::step` — which polls the pad through `input::Manager` and steps
  `game_sim::ride::Ride` on the disc's own `.phy` — followed by `game_sim::camera`'s follow camera,
  the draw and the present (P2). The ticks run on their own 30 Hz schedule and the picture between
  them, as fast as it can (P52). `--record` writes every simulated frame's pad to
  a tape and `--play` rides one — a tape of the shell or a human's `.p2m2` — at the tape's own frame
  lengths, so a control complaint is a file somebody else can ride (P7).
- `pace.rs` — the ride's clock (P52): ticks on the original's 30 fps schedule whatever the drawn
  rate, each reading the Timer 0 a steady PS2 reads, a catch-up cap, and the render-side blend
  between the last two ticks.
- `window.rs` — a window with no dependencies: the X11 wire protocol over the display's unix socket,
  a window, a graphics context, keyboard events, and the frame pushed across as one `PutImage` a band.
- `pad.rs` — the host's pad: Linux's joystick events and the keyboard, folded into the 32-byte
  DualShock 2 report the ported input layer reads, so the port's own poll is what the game sees.
- `ground.rs` — the freeride area's ground: the ported terrain query asked about a world point rather
  than a sample, and the samples around the rider turned into triangles, because the renderer's `.trn`
  reader does not read an off-road layout.
- `hud.rs` — the session markers' slot display a view at a time (`MarkerBar`, `MarkerBanner`), which
  the ride HUD's marker menu draws. It was the debug HUD (`--hud`, P6) until P170 took that out.
- `moment.rs` — `--moment FILE OUT.png`: one frame posed from a moment file (a bike's transform and
  the frame's camera, read out of a save state by `research/shots/moment.py`) instead of from the
  simulation, so the port's picture can be put beside the original's (V2).
- `track_previews.rs` — the track pick's card pictures for mod tracks (P70): a mod's own
  `previews/track.png`, else a still of its spawn the shell's binary takes headless in the
  background and keeps in `mods/previews/`; and the small PNG reader both are read with.
