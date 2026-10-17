# 1 — replay seam (FINAL)

Task P5. 2026-10-17.

## What I did

The harness and the shell now drive the same value. `Sim` — the one `impl Game` — stopped being A1's
example ball (`ticks`, `rng`, `physics::Ball`) and became the thing a person rides:

```
Sim { ticks, rng, tables: Arc<Tables>, phy: Phy, input: input::Manager, rider: rider::Rider }
```

- **Three constructors, because three callers need different amounts of say.** `Sim::new(seed)` is a
  bare state for a test or a fixture: the stand-in bike from the new `stock` module, dropped on a level
  floor at `stock::SPAWN`. `Sim::ride(seed, tables, phy, at, facing, set)` places that bike somewhere.
  `Sim::with_rider(seed, tables, phy, rider)` takes a `Rider` somebody else built — which is what the
  shell needs, because it takes the wheel mounts off the bike *model's* own nodes rather than off the
  `.phy` (S22) and turns the wreck test on and off from its command line.
- **The world is an argument, not a field**: `Sim::frame(dt, reports, &mut dyn World)`. The shell's
  world borrows the disc bytes it was loaded from, so a `Sim` that owned one could not be `'static`,
  could not be cloned for a fixture and could not be built from a seed alone. `Sim::tick`/`tick_at`/
  `tick_frame` are the same frame over `world::Flat` — a floor and no props — which is what a replay
  fixture runs on and what `impl Game for Sim` calls.
- **The pad is inside the state.** `Sim` owns the `input::Manager`, because `ride::Controls::from_pad`
  reads press edges and so needs the layer that remembers the last frame. `Sim::poll` hands back the
  `Controls` for a frame and `Sim::step` runs one; `tick_frame` takes the twelve pressures and the
  measured `dt` a `replay::PadTape` carries, and `tick_at` the narrow `Input` a fixture carries.
  `replay::Playback` is now one field (`sim`), and its `input()` is the sim's own.
- **The walk names the capture's columns.** `impl State for Sim` walks `ticks`, `rng`, then a group
  `bike { pos, vel }` — aliases of `rider.ride.origin` and `rider.ride.chassis.vel` — then `input` and
  `rider`. That is the eight names `BIKE` in `tests/replay.rs` maps `bike_xform.m30..m33` and
  `bike_vel.x..w` onto, so an acceptance capture of the original is a real comparison instead of
  `NoSuchField 'bike.pos[0]'` at tick 0.
- **The shell keeps one `Sim`** in its `World` (`crates/openmx/src/main.rs`): `poll`, the Start toggle,
  then `step` into the loaded place. Everything that used to reach a loose `rider`/`manager`/`tables`
  now reads `world.sim.*`. Its `--record`/`--play` tape rides unchanged.
- `tick.rs`'s `impl Game` is three lines (`tick_at`, then `true`), `rider::Rider` derives
  `Clone, Debug`, and `Sim`'s own `Clone`/`Debug` are by hand so the `Arc<Tables>` is shared rather
  than copied and a `{sim:?}` in a test assertion does not print a megabyte of tables.

## What the original does

Nothing new was read out of the decompilation for this one: the frame it runs is S3's movement, S10's
air, S11's collide and S20's state machine as they already landed, in the order `rider::Rider::frame`
already drove them. The one claim about the original that this task does keep is the *order* inside a
frame — the pad is read once, before the simulation, and the simulation sees the layer's state rather
than a bitmask — which is why `Playback` reads the tape's frame into the manager and then steps, and
why the manager lives in the state being hashed. That order is I2's finding, not this task's.

The stand-in's own numbers are not the original's: `stock::BIKE` is hand-written `.phy` text, not a
file off the disc (see below).

## How it was proved

- `./kit check`: **TESTS PASSED** (503 in this worktree; the total is shared with whatever else is in
  the tree, so the verdict is the number that means anything). Full log `context/notes/check.log`.
- The replay suite (68 tests) is green, and its acceptance half now proves a comparison rather than its
  absence:
  - `the_acceptance_run_compares_the_bike_the_port_rides` (replaces
    `the_acceptance_fixture_names_the_field_the_port_is_missing`): a 60-frame synthetic capture in
    `watch/acceptance.toml`'s exact shape verifies `Ok`, and one bit moved in `bike.pos[1]` on tick 37
    fails as `Field` at tick 37 naming `bike.pos[1]` — a tick and a field, which is P5's acceptance
    criterion.
  - `a_capture_of_real_play_becomes_a_fixture`: a 240-frame capture replays tick-exact **with its own
    paths untouched** (asserted equal to `AS_THE_PORT_HAS_THEM`), where before it had to be re-aimed at
    the ball's paths to replay at all.
  - `acceptance_csv` is two passes now, because the bike is driven by the pad: the first writes the
    capture with its bike columns zeroed and reads it back, the second fills those columns with the
    port's run on *that capture's own* derived inputs and seed. A reader that dropped or shifted a
    frame of input would no longer produce the bike it recorded.
- Sensitivity, through the harness: `a_divergence_in_any_field_fails_the_replay` — a watched lane is
  caught at tick 0 and an unwatched one by the first hash at tick 1.
- The shell, on the real window (`DISPLAY=:0`, 320x240, `--seconds 3`, Supercross01): recorded a
  180-frame tape and played it back. Both printed `ride hash 0x3140102cddb3e085` — the rewired shell
  rides, and P7's tape round-trip is still bit-exact through the new seam.
- Cross-architecture pins regenerated for the new state (`tests/platforms.rs`, via
  `print_the_vectors`): `HASHES` from `(16, 0xfa4ecba77d27cd4b)`, `TRACE_DIGEST 0xe2a6a588f32b9276`,
  `FINAL_DUMP_LEAVES 809`, `FINAL_DUMP_DIGEST 0x2b2025a68e851feb`,
  `FIXTURE_TEXT_DIGEST 0x2cacd5d7e8a24271`, and the hundred-tick hash `0xd53f_5712_514b_9c0c` (pinned
  in `src/lib.rs`, `tests/hash.rs` and `tests/platforms.rs`). `tests/hash.rs` pins the walk's head (ten
  leaves: `ticks`, `rng`, the eight bike lanes) as a hand-written byte stream, plus
  `size_of::<Sim>() == 5360` and 749 leaves in a fresh dump.
- What a bare seed actually is, written down as a test
  (`the_stand_in_bike_is_dropped_onto_the_floor`): dropped a foot onto a level floor, both wheels take
  the landing, phase stays `Riding`, and it settles at y = -0.214 (the chassis' own reference point sits
  below the floor plane) and does not move for another 300 ticks.

## Not verified / not 1:1

- **The acceptance verdict itself is unrun.** `context/` is per-worktree and this worktree has no
  acceptance capture, so the one thing P5 exists to enable — the real `the_acceptance_run` against
  S8's 1,810-frame capture of the human's minute — has not been executed by me. Everything proved
  above is a synthetic capture of the port's own numbers.
- **`acceptance_verdict` still panics on a `Field` divergence**, and that is now reachable. The policy
  question (does the gate fail or report when the port disagrees with the original?) is S8's, not
  P5's, so I left the code alone and wrote it into the test's own section comment: expect the first
  real verdict to be a divergence, because the ride's numbers are a stand-in's until D7's parameters
  and the remaining feel tasks are 1:1.
- **The stand-in bike is not the disc's.** `stock::BIKE` is hand-written `.phy` text, duplicated from
  `crates/game-sim/tests/freeride-bike.phy.txt` because a crate cannot read a test's file at run time
  and the real file is game data. Every fixture the port records against itself therefore starts from a
  bike nobody shipped. `stock::SPAWN`/`FACING` are a point at the origin a foot up and +Z, not a
  track's start line.
- **`Sim::new` turns the wreck test off** (`rider.sense.wrecks_enabled = false`), which is the
  stand-in's doing and not the original's. With it on, a bike placed on a start line — no velocity,
  nothing asked of the ground yet — answers `air::Wreck::NoseDive` (8) on its *first* frame with
  `g_peak` zero, and a down rider is not integrated, so the state a bare seed builds would stop being a
  ride on tick 0 and every self-recorded fixture would be a frozen wreck. Reproducer: `Sim::ride` with
  `wrecks_enabled = true` → `Wreck(8)` at tick 0, position stuck at 0.9530511 with velocity
  -1.3415323 and a respawn every ~160 ticks. **A finding for whoever owns the airborne test (S10):
  either the first-frame guard is missing or the test needs a contact the spawn has not had yet.** The
  shell is unaffected — it sets the flag from `--wrecks`/`--no-wrecks` itself.
- **`phy` and `tables` are deliberately absent from the walk.** They are the bike's and the game's
  constants, not state; a divergence caused by loading a different `.phy` shows up as moved numbers on
  the first tick rather than as a differing hash of the parameters. The consequence is that a fixture
  cannot tell "the same ride on a different bike" from "a different ride".
- **`bike.pos`/`bike.vel` are aliases.** The same two quads are walked again inside `rider`, so they
  are hashed twice; the walk's leaf count (749 fresh, 809 after a run — it contains lists) is not a
  count of distinct state.
- **`Sim::reports` is lossy**: it turns an `Input`'s bitmask into full-pressure digital reports, so a
  fixture in the narrow `Input` form cannot express an analogue strength. A capture played through
  `tick_frame`/`PadTape` can.
- **The fixture format carries 32 bits of the original's 64-bit RNG state** (`seed`), unchanged from
  A2 — an acceptance fixture's start state is therefore not the original's to the bit.
- **Nothing here compares a number against the emulator.** No PINE session, no capture, no oracle run:
  this task is a seam, and every number it moved is the port's own.

## Next

For S8, with no code to write in `game-sim`: run `./kit check` in the worktree that holds
`context/capture/acceptance-freeride.csv` and read the first real verdict. It will be a tick and a
field; decide there whether `acceptance_verdict` panics or reports, and start the list of what diverges
first. For whoever owns S10's airborne test: the `NoseDive`-on-the-first-frame reproducer above, which
is the one finding in this task that is about the port's behaviour rather than its plumbing.
