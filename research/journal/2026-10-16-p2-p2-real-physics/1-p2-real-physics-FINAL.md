# 1 — p2 real physics (FINAL)

Task P2. 2026-10-16.

## What I did

The shell had its own physics: `crates/openmx/src/ride.rs`, 399 lines of invented springs, invented
gravity and a stiff camera placement, written so there was something to look at while the real
modules were being ported. All of it is gone. In its place the ride is a `game-sim` state driven by
the ported modules, on the parameters S18 reads off the disc, sweeping S4's collision against the
scene's own placements, with S7's camera behind the view.

- **Deleted** `crates/openmx/src/ride.rs` and its constants (`git rm`), and the stand-in ground query
  that fed it — `ground::Surface`, `ground::Contact` and `Ground::at` in `crates/openmx/src/ground.rs`
  (the drawing half of that module is untouched).
- **New `crates/game-sim/src/ride.rs`** (1630 lines): the whole bike as one value. It holds the
  canonical `chassis::Chassis`, the engine, the air state, two wheels and the rider's hands, and
  projects the four views of the chassis (`move::Chassis`, `wheel::Chassis`, `air::Chassis`,
  `chassis::Chassis`) and the two of each wheel that the four proved modules each want. `Ride::frame`
  rewrites `move::step`'s spine inline and *calls* the real callees where `move::step` only records
  them: the substep loop, both suspensions, both tire force passes, both wheel integrations,
  `engine_update`, the drag, the reverse push, the collision sweep and `chassis::integrate`.
- **New `crates/game-sim/src/world.rs`** (415 lines): the one seam between the ride and the track.
  `trait World` is three questions — where the ground is under a point, what surface it is, and what
  the collision sweep hits — with `Flat` for tests and `Place { ground, props }` for a real scene.
  `Props::load` stands every collision mesh the scene names where the scene puts it (the mesh's own
  transform, then the placement's basis) and `Props::sweep` runs S4's sweep against the lot, in each
  prop's own space, merging the hits back into one world-space record.
- **`math::quat_to_euler`**, the conversion the chassis measures its pitch and roll with, and
  **`camera::Preset::read`/`read_bytes`**, the `cpresets.ini` loader, so the shell's camera is S7's
  follow camera on the disc's own presets rather than a fixed offset.
- **Two constants made `pub` in `move.rs`** (the idle speed the clutch rule reads and the distance
  that counts as a near collision). Nothing else in the four modules this task drives was touched.
- **`Controls::from_pad`** in `ride.rs`, so the shell and a replay fixture derive the rider's hands
  from a pad through one function instead of two copies of the same bindings.
- **A bug the shell could not have shown**: the ported engine made **no torque at all**. The
  engine's constructor sets six fields to 1.0 (`research/oracle.txt`, S15 block), and two of the six
  are not `.phy` keys, so the loader cannot write them: the clutch's launch multiplier and the scale
  on the torque leaving the gearbox. The port's `Engine` starts zeroed, so `engine_update`'s
  `torque = clutch_torque_scale * …` and `wheel_torque = torque_scale * …` were both identically
  zero and the bike was rolling on gravity alone. `Ride::new` now starts the engine the way its
  constructor does. This is the one correction this task makes to somebody else's proof, and it is
  outside the proved function: the oracle *varies* `torque_scale`, so `engine_update` was right —
  nothing had ever constructed an engine for it.

- **Wrote two findings into `research/oracle.txt`** (a new P2 block at the end): the order of
  `chassis_integrate`'s two transform calls, which decides whether the chassis' origin is built from
  the previous rotation or the new one, and the engine constructor's two words that no `.phy` key
  writes — the bug above, written down where the next person will find it.

## What the original does

Recipes, not names: `./kit fn bike_ground_step`, `./kit fn bike_phy_load`, `./kit dis <addr>` for the
engine constructor named in `research/oracle.txt`'s S15 block.

- **The frame** is A5's order, written down in `research/oracle.txt`: the ground step, then the air
  state, then the attachment walk, then the stunt clocks, then the ride state machine. This task
  drives the first two and the third only as far as refreshing the bike node's axes; the last two are
  S5's and P3's.
- **The ground step** runs `(int)(55 · dt) + 1` substeps of `dt / n`. Inside a substep the order is
  load, the rider's four controllers, the air code's three, the engine, the drag, the reverse push,
  `wheel_clear`, the contacts (which get the *frame's* `dt`, not the substep's), the two suspensions,
  the two tire-force passes, the two wheel integrations, the engine's post pass, the collision record,
  the chassis' measure and the integration. S3 and S17 proved that order against the original; this
  task reproduces it and is tested against the proved recording of it (below).
- **The engine's constructor** zeroes the object and then writes 1.0 into six words and 1 into the
  gear. Two of those six are never written again by anything found: the clutch's launch multiplier
  (the launch ramp owns it from the first frame the lever is out) and the gearbox output scale, which
  S15 looked for a setter for and did not find.
- **The collision** is one sweep of a bundle of probes against the scene, and what comes back is the
  travel each probe did *not* get to make, laid along the contact normal. The ride spends that on the
  chassis' impulse and kills the velocity into the normal, which is what stops the rider at a prop.

## How it was proved

Not a function oracle: `ride.rs` is not a port of one function but the wiring of five that are ported
already, and there is no retail address to run against the wiring. So the proof is of the properties
wiring can get wrong, in two files.

**`crates/game-sim/tests/ride.rs`** (new, 6 tests, on a bike written in
`crates/game-sim/tests/freeride-bike.phy.txt` — hand-written, because the disc's `.phy` cannot be
committed):

- `the_frame_hands_its_callees_out_in_the_ground_steps_own_order` — **90 frames**, each one run twice:
  once through `Ride::frame` and once through `move::step`, which S3 and S17 proved against the
  original over thousands of generated states. Every call the frame makes outside itself is compared
  against the call `move::step` recorded, in order, with its `dt`. Payload vectors are deliberately
  not compared (see *Not verified*).
- `the_frames_list_is_the_substep_loop_it_claims_to_be` — one substep at 60 Hz and six at 100 ms, the
  frame's `dt` on the contacts call and the substep's everywhere else, the collision record opened on
  the first substep only.
- `the_same_inputs_ride_to_the_same_hash_twice` — **600 frames**, two separate states, identical
  per-tick hash lists, and more than 500 distinct hashes so that a frozen bike cannot pass.
- `a_divergence_in_the_ride_is_named_by_its_field` — one lane of one field changed, and the walk names
  `chassis.vel[2]`.
- `a_prop_stops_the_rider` — a rider thrown at a wall built in code and stood through
  `world::Prop::place` ends up on this side of the wall plane with the body-contact flag set, and the
  same throw with the wall removed passes straight through where it was, so the assertion is about the
  wall and not about drag.
- `the_walk_names_every_field_once` — every leaf path distinct, the leaf count (471) and
  `size_of::<Ride>()` (1800) pinned, so a field added to the ride without a line in the walk fails
  here instead of hashing to the same value for ever.

**`crates/game-sim/tests/replay.rs`** (4 tests appended, the brief's `t:` line):

- `a_freeride_fixture_replays_to_the_same_hash_twice` — ten seconds of pad (clutch, throttle, the
  stick working, the brake, a trick asked for) recorded as a `Fixture` with a hash every tick and
  eight watched fields every thirtieth, then ridden again and compared: **760 checks**, and the
  recording itself reproduced field for field.
- `the_freeride_fixture_is_a_ride` — more than 80% distinct hashes, and the bike moved, changed speed,
  changed revs and moved its suspension.
- `a_freeride_fixture_round_trips_as_a_file` — through `to_text`/`parse`, floats and all, and the
  fixture read back off the file replays.
- `a_diverging_freeride_fails_at_the_tick_it_diverged_on` — a nudge at tick 150 fails at tick 150 with
  the field named; with the hashes stripped out the watches catch it on their own at the next sampled
  tick.

`./kit check`: **TESTS PASSED: 439 tests**.

**The acceptance ride.** `cargo run -p openmx -- --seconds 60`:

```
Supercross01[Quadzilla]: 577 props, 353 collision shapes, ground 6903 x 6903 feet in 768 x 768 samples
solid: 353 of 353 collision meshes
loaded: 133 of 133 models, 24 textures, bike
start: 2884, 338, 2750 facing 0 degrees, 11 camera presets
the dirt under the start line is at 336.8 feet
…
ridden for 60.0s over 3583 frames
```

Sixty seconds, 3583 frames, no panic, at 59.7 fps with all **353** of the scene's collision
placements loaded and swept. No emulator was needed for any of this, so none was started.

## Not verified / not 1:1

- **The frame itself has no oracle.** What is proved is that it calls what `move::step` says to call,
  in that order, with those arguments, and that the result is deterministic — not that the bike's
  state after a frame is the original's. That needs a PINE recording of a freeride compared frame by
  frame, which is P3's acceptance and not this task's.
- **Call payloads are not compared** against `move::step`'s recording, only the call names and their
  `dt`. Over a void world the two spines legitimately diverge in the vectors: the ride integrates the
  wheels and the chassis for real where `move::step` only records that it was asked to, so by the
  second substep the two are in different states. The *order* is the thing that can silently rot, and
  the order is compared.
- **Five engine helpers are the port's own answers, not ports.** `spin_integrate`, `surface_drag`,
  `hinge_force`, `contact_velocity` and `set_axes` are written from their disassembly's description
  rather than proved against it; each is marked in `ride.rs` where it is used. `quat_from_axis` and
  the `noise` the balance controller reads are the same.
- **The attachment walk is not ported.** The bike node's axes are refreshed in place of it (the node
  keeps its own up axis once it has one, which is what the no-hander writes). A rider, a seat and the
  rest of the walk are A5's and S20's.
- **The two pass patterns are equivalences, not transcriptions.** `chassis::integrate`'s `Given` is
  iterated to a fixed point (four passes) and `wheel::tire_forces` is run on a copy first to answer
  its second velocity question and its deferred hinge force. Both are argued in comments at the call
  site; neither is proved to be what the original's single pass computes.
- **The bundle's composition is the port's own**: two suspension probes, the chassis' travel this
  frame and a downward reach. The original builds its bundle inside a function nobody has read.
- **The three zeroed contact words.** A contact's node, owner and vehicle words are zero: they belong
  to the object graph (who you hit, and what kind of thing it is), and nothing ported reads them.
- **S21's 25 zeroed `Params` fields are why the ride is soft.** The disc's `.phy` keys for them are
  read, the fields exist, and the controllers that read them are proved — but the keys were not in
  the loader's table when S18 landed, so a quarter of the tuning arrives as zero. A peer's standing
  instruction is not to fix it inside P2. Until S21 lands, the bike understeers and the balance is
  weak, and no test here asserts a value those fields decide.
- **The engine's two constructor ones** (above) are read off the constructor's disassembly as recorded
  in `research/oracle.txt`, not proved by running it. The other four of the six are already written by
  the loader or by `set_gear`.
- **`MAX_SPEED` / `MAX_ANG_SPEED`** in the chassis' integrator limits are stand-ins wide enough that
  neither can fire; the original's come from the vehicle's setup, which is not ported.
- **The input layer's binding table is the port's own.** Which button is the throttle is this
  project's choice (`Controls::from_pad`), as is the eight-way d-pad code and the fold of one brake
  lever into the front and rear words S17 proved are separate. The rate limits themselves are the
  disc's.
- **The stick's angle and deflection** that the donut reads are the port's own reading of what the
  donut's proved ramps require, not a port of the front end.
- **The camera's collision probe is not ported**: the follow camera is handed a nearest-obstacle
  distance of zero every frame, which is "nothing in the way". `Follow::new`'s reverse-speed threshold
  is taken from the first preset's own forward/reverse switch speed, because the original's call site
  was not read. The horizontal-to-vertical field-of-view conversion assumes the disc's 4:3 viewport.
- **The terrain is parsed twice** in the shell, once for drawing and once for the simulation, because
  the two sides of the file are owned by different crates. The ground query itself is `world::Ground`,
  whose blend between samples is the port's own.
- **The test bike is not a real bike.** `tests/freeride-bike.phy.txt` is hand-written in the game's
  own format and read by the ported loader; no number in it is claimed to be the game's, and no test
  asserts a value its parameters decide. On it the ride bounces more than a game bike would — which is
  a statement about the file, not about the port, and is why the fixture tests assert determinism,
  order and structure rather than trajectories.
- **The 60 s ride was watched through its own log, not looked at.** This desktop has no GUI
  automation and screenshots of a window need the application to write them; the acceptance claim is
  "60 s, 3583 frames, no panic", not "it looked right".
- **`ride.rs` walks its own state with private helpers** rather than `impl hash::State` on
  `move::Ride`, `chassis::Chassis`, `air::Air` and `wheel::Wheel`. That is deliberate — this task adds
  nothing to four other tasks' modules, so the merges stay clean — but it means those four structs
  still have no walk of their own, and a field added to one of them is invisible to the hash until
  somebody adds a line here. The leaf-count guard in `tests/ride.rs` is what catches that.

## Next

Nothing waiting on the human. In order of value:

1. **S21** — the 25 `.phy` keys the loader drops on the floor. Everything else in the ride is proved
   or argued; the parameters are the reason it does not feel like the game yet.
2. **P3** — the ride state machine's clocks and the respawn behind `Ride::place`, and the PINE
   recording of a freeride compared frame by frame, which is the proof this task could not write.
3. The five engine helpers listed above, each of which turns a *Not verified* line into a proof.
