# PLAN — how work gets done here

`AGENT.md` has the rules. This file has the mechanics: how a task goes from the list to a commit, and
where everything lives so you never have to search for it.

## Continue

"Continue" is one task, start to finish:

```sh
./kit plan next                 # the first open task
./kit plan brief T7             # its line, its own file, its journal, the files it names
                                # ... do the work ...
./kit check                     # tests
./kit journal new T7 walker     # the entry, including what you could not verify
./kit done T7 "port the walker" # tests again, tick the task, commit, push
```

Then stop and report what you did and what you did not verify. One task per iteration. Don't merge two
tasks because they look related, and don't skip ahead to a more interesting one.

If a task turns out to be bigger than one sitting, split it before starting: write `plan/<ID>.md` files
for the parts, `./kit plan add` them, then take the first.

## The task list is a tool, not a file

Don't read `plan/TODO.md` and don't edit its checkboxes by hand. It is a plain-markdown file so a human
can skim it, but every change goes through the command, which keeps the format consistent and the archive
in step:

| Command | Does |
|---|---|
| `./kit plan` | the index: sections and how many are open in each |
| `./kit plan next` | the first open task |
| `./kit plan T7` | that task: its own file if it has one, else its line |
| `./kit plan brief T7` | everything you need to start it, in one call |
| `./kit plan tick T7` | done → moves it to `plan/DONE.md` |
| `./kit plan block T7 "<why + what unblocks it>"` | blocked, with the reason recorded |
| `./kit plan unblock T7 [note]` | un-blocked |
| `./kit plan add T "text" [@ where]` | a new task, next free ID in that prefix |
| `./kit plan move T7 top\|before T3\|after T3\|in <section>` | reprioritise |
| `./kit plan note T7 "<text>"` | add a line to its file |
| `./kit plan newid T` | the next free ID in a prefix |
| `./kit plan archive` | sweep finished lines into `plan/DONE.md` (after merges or hand edits) |

`brief` is the one that matters. It assembles the task's line, its plan file, its journal directory (the
newest entry pasted in, capped), and each file the task names with its length and last few commits. One
call instead of eight, and you start with the same context every time.

Why a tool and not a file: a task list you read is a task list you read *all of*, every turn, for the
rest of the session. The list is a few hundred lines; `./kit plan next` is three.

## The journal

Every task gets an entry in `research/journal/YYYY-MM-DD-<ID>-<slug>/`. This is the project's memory: what you
found, what the original does, which addresses you used, what you measured, and — the important part —
what you could not check.

- One README per task. Newest iteration file names whose move it is: `-FINAL` (done), `-PART` (an agent
  continues), `-READY` (needs the human).
- `./kit journal new <ID> <slug>` creates the directory and stubs with the required sections;
  `./kit journal state <ID>` answers "whose move is it" and `./kit journal finish` closes an entry.
- Captures, screenshots and dumps go in `context/`, **never** in the journal — the journal is committed.
- The last section is always `## Not verified / not 1:1`, with a reason per item, or `none`. If this
  section is missing, `./kit done` tells you so.

The journal is why a project like this can survive being picked up months later, or by someone else, or
by an agent with no memory of yesterday. Write it for that reader.

## Wrap-up

```sh
./kit done T7 "port the walker"                  # tests → tick → commit, one call
./kit done T7 "partial: tables read" --blocked "needs a save state in the second stage"
```

It runs the test command, runs the float lint if the port has one, refuses to commit if the tests fail,
ticks or blocks the task, commits everything including the journal entry, and pushes the branch it
committed on. One call, so a wrap-up cannot be half-done — the state where the code is committed but the
task is still open is the one that confuses the next session most, and the state where it is committed but
only on this disk is the one that loses work outright.

Pushing your own branch is nobody's manual step: `./kit done` pushes the branch you are on when it is a
task's own (an agent's `task/<ID>` from its worktree, a topic branch here). The **shared** branch is pushed
only by `./kit run`, after a merge has passed the test gate (`[git] push_merge`, on by default); a wrap-up
in the main checkout prints the one-line `git push` instead. A merged `task/<ID>` is dropped from the remote
once the remote holds the merge. A push that fails is a logged line and nothing more. Turn it all off with
`[git] push = false`.

## Code map

Keep this current. It exists so nobody greps the repo to find where something lives.

### The kit

`ps2kit/` — the tooling. All of it generic; see `docs/INTERNALS.md` for the contract.

- `config.py` reads `kit.toml` (the only game-specific file) · `platform_.py` the whole OS surface
  (the only OS-specific file) · `cli.py` dispatch
- static: `disc.py` ISO 9660 · `elf.py` PS2 ELFs and overlay ELFs · `ghidra.py` headless analysis ·
  `decomp.py` read the dumps one function at a time
- live: `pine.py` PCSX2 memory · `pad.py` virtual controller · `emu.py` launch/stop/screenshot/states ·
  `probe.py` find addresses · `record.py` declarative captures
- workflow: `plan.py` · `journal.py` · `checks.py` · `runner.py` + `runner.toml` the kit's settings for
  [agent-runner](https://github.com/FIZL0/agent-runner), which runs `./kit plan` and `./kit run` · `oracle.py` ·
  `setup.py` setup and doctor · `hooks/` the context guards

`watch/` — capture specs (`*.toml`), one per thing you record. `research/` — your own scripts and notes,
committed, no game data. `context/` — everything game-derived, git-ignored.

### The port

`crates/` by default, but this is yours to shape. The three that come with the kit, and what this port has
added beside them:

- `ps2-float` — the PS2's arithmetic: EE FPU add/sub/mul/div/sqrt/madd, VU0 macro ops, the game's own
  transcendentals. Everything in the simulation goes through this.
- `ps2-oracle` — a small R5900 interpreter that runs one function from the game's own code so a test can
  compare your port against it over thousands of inputs.
- `game-sim` — where your port goes. Starts nearly empty, with the replay harness and the state hash,
  because those two are what keep everything after this honest.
  - `hash.rs` — the state hash: one walk over the whole state, folded into 64 bits for a per-tick
    comparison or dumped field by field to name the first field that differs (A1). Every state type
    describes itself once, with `impl hash::State`.
  - `input.rs` — the input layer: the pad read once a frame before the simulation, mapped into the
    game's own bitmask and kept as sixteen controls a pad — held, strength, the press and release
    timers — plus the one-frame event queue the front end reacts to. The original keeps no "pressed
    this frame" mask, so the edge is the event and "held with the timer at 0" (I1). The same poll's
    six analogue axes: the report byte each one reads, the inversion one of them gets, and the dead
    zone of ±0.2 that is the only shaping there is — no clamp and no curve (I2).
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
- `game-render` — the drawing side: the disc's own models and textures, on screen. Host `f32`, no
  bit-exactness, nothing the simulation depends on.
  - `disc.rs` ISO 9660 → `MX.PSR` → a `.res` member, read from the user's disc at runtime ·
    `psg.rs` `.psg` geometry: nodes, materials, LOD groups, VIF streams → triangle strips ·
    `pst.rs` `.pst` textures: a 256-colour palette in the GS's order, plus its mip chain ·
    `terrain.rs` `.trn` ground: a patched height field, its palette and its ground texture
  - `ini.rs` the plain-text format the disc's data files share · `scene.rs` `.scn` scenes, `.psm`
    materials, `.sub` bike builds and the `cpresets.ini` follow cameras · `world.rs` a track in
    memory: models, materials and textures matched by name, and drawn
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
- `openmx` — the playable shell: the binary a person rides. A window, a real pad, the ported simulation
  at the game's own 60 Hz, and the freeride area drawn with the renderer, with no emulator, no fixture
  and no menu in the way (P1).
  - `main.rs` — the loop: the disc's track and bike, the scene read twice (the simulation's reader for
    the spawn, the terrain and the collision placements, the renderer's for props, lights and fog),
    then a frame of `tick::Engine::step` — which polls the pad through `input::Manager` and steps
    `game_sim::ride::Ride` on the disc's own `.phy` — followed by `game_sim::camera`'s follow camera,
    the draw, the present and the pace to 60 Hz (P2). `--record` writes every simulated frame's pad to
    a tape and `--play` rides one — a tape of the shell or a human's `.p2m2` — at the tape's own frame
    lengths, so a control complaint is a file somebody else can ride (P7).
  - `window.rs` — a window with no dependencies: the X11 wire protocol over the display's unix socket,
    a window, a graphics context, keyboard events, and the frame pushed across as one `PutImage` a band.
  - `pad.rs` — the host's pad: Linux's joystick events and the keyboard, folded into the 32-byte
    DualShock 2 report the ported input layer reads, so the port's own poll is what the game sees.
  - `ground.rs` — the freeride area's ground: the ported terrain query asked about a world point rather
    than a sample, and the samples around the rider turned into triangles, because the renderer's `.trn`
    reader does not read an off-road layout.
  - `hud.rs` — `--hud` (H toggles it): the riding overlay, drawn with no triangles — a 2-D blit into
    `Frame::color` after `draw_scene` and a 3x5 font — showing the sixteen controls as `input::Manager`
    holds them, both sticks where the dead zone put them against their raw report bytes, and the
    numbers the ride is running on. It matches nothing on the disc and draws nothing with the flag
    off (P6).
  - `moment.rs` — `--moment FILE OUT.png`: one frame posed from a moment file (a bike's transform and
    the frame's camera, read out of a save state by `research/shots/moment.py`) instead of from the
    simulation, so the port's picture can be put beside the original's (V2).

As you add modules, list them here with one line each, the way `ps2kit/` is listed above. A code map that
is a sentence per file is worth more than a directory listing, and it is the cheapest context in the
project.

## Unattended runs

```sh
./kit run once                  # one task, then stop
./kit run 3                     # three agents at a time, each in its own worktree
./kit run 3 --mode p            # ... strictly in TODO.md order (mode d, the default, keeps them off each other's files)
./kit run --dry-run 3           # what would start, and why the rest wait
./kit stop                      # running tasks finish and merge, nothing new starts
```

The loop is [agent-runner](https://github.com/FIZL0/agent-runner) (install: SETUP.md step 1). Each agent
gets a git worktree on a `task/<ID>` branch and its own emulator copy (`KIT_PCSX2=N`), and each branch is
merged in a separate merge worktree when its session ends, tested, and pushed.

**A conflict the runner cannot join goes to an agent session, not to you.** Files where a line is a whole
record (the task list, the journal, the oracle's address list) are joined mechanically; a name given two
addresses in the address list is refused (`./kit oracle join`). Anything else gets one session per merge in
the merge worktree, which resolves and commits; the runner then tests it as usual. A branch that session
gives up on is parked as `wip/<ID>` and the task opens again. `[run] fix_merges = false` parks them all.
See `docs/WORKFLOW.md` for what to expect from a long run and how to pick up the pieces afterwards.

## When blocked

Stalled means: a tool hangs, the emulator will not cooperate, the same fix fails twice, something needs a
human, or twenty minutes pass with nothing to show. Then: stash the experiments, write what happened in
the journal, `./kit done <ID> "<what is solid>" --blocked "<why + what unblocks it>"`, and take the next
open task — preferably in a different section, since the obstacle is often shared within one.

Never end a run while open tasks remain. At the end, list the blockers. Human-only items go in
`plan/HUMAN.md` — never into the code as a guess.
