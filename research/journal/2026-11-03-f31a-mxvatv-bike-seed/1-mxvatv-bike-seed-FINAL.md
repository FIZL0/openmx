# 1 — MX vs. ATV bike seed (FINAL)

Task F31a. 2026-11-03. Slot s3.

## Decision: what "a Ride seeded from the bike" is

The brief asks for a `game_sim` Ride on `Handling::MxVsAtv` seeded from state 4's bike. `ride::Ride`'s
body is MX Unleashed's chassis (`chassis.rs`: cofg at +0x60, quat at +0x1a0, its own integrator).
SLUS-21104's bike body (bike +0xb8) is not that object. It is the engine's later dynamics body, the one
MD75 ported for the quad as `atv_body::Body` (vtable 0x003cd200, MD75's map: quat at +0x190, inverse
inertia at +0x140). State 4's body reads exactly that way. The bike rides SLUS-21104's physics world as
the quad does (the quad is "the bike with four wheels", `quad.rs`).

So the seed is in the types SLUS-21104's ports already use, not in `ride::Ride`. Seeding Ride's chassis
would have meant mapping one integrator's state onto another's, and no frame of Ride's could then be
bit-exact. F31b's per-frame bike sim is the place that builds the riding object around these types.

## What I did

- `crates/game-sim/src/handling/seed.rs` (new, `pub mod seed` in handling.rs). `seed::read(words, bike)`
  reads the bike at `bike` into a `Seed`:
  - the body (`atv_body::Body`, all of MD75's fields);
  - both wheels (`wheel::Wheel` at the ATV's offsets, the same fields `tests/atv_quad/image.rs` reads
    for the quad);
  - the engine (`r#move::Engine` at MD75b's offsets: gearbox, clutch, curve cursor, last outputs, and
    the ratio, revs and torque tables).

  Every reader has a `*_words` inverse (address, word), and `Seed::words()` gives all of them. Floats
  are read raw: `F::from_bits` flushes a denormal, and the engine's tables past their 10 points hold
  words that are denormals as floats.
- `crates/game-sim/tests/mxvatv_bike_ride.rs`, two tests added (the F31 ones are unchanged):
  - `the_seed_is_state4s_bike_word_for_word`: all 288 words the seed stands for are the image's. The
    bike is enabled, both wheels touch and have ground, and the engine is in gear and turning.
  - `the_seeded_bikes_first_frame_is_ride2_row_0`: one sim frame (vt +0x40) is run in pieces.
    - The +0x24 list's vt +0x7c then vt +0x84, the tail and the +0x3c list run in the interpreter.
    - The world step is the port's (`biplane::World::step`, the image's 1 iteration at 115/s), and
      the seeded body's integrate and end of substep are the port's (`atv_body::Body`).
    - The world's listeners (6 preforce, 1 force, 1 post) are still SLUS-21104's own code. The
      interpreter runs them on the port's body: it is written into RAM before each listener and read
      back after it.
    - The result: four substeps. The port's body is ride-2 row 0's body block word for word. The node,
      collision record, front and rear wheel and engine the listeners leave are row 0's word for word
      (the F31 replay's `diff`).
- research/oracle.txt `# F31a`: the sim frame's pieces, state 4's world lists and their getters, the
  engine's tables.

## How it was proved

- `cargo test --release -p game-sim --test mxvatv_bike_ride`: 5 passed (the 3 F31 tests and the 2
  new ones). The image and the C15 captures are in s3's context/, so nothing skipped. The debug build
  passes too.
- Mutations, each run and then reverted:
  - the port's integrate skipped in the hybrid frame: the body parts from row 0;
  - the seeded body's velocity.x one ulp off: the body parts from row 0.
- `cargo check -p game-sim --tests`: clean (only warnings already there).
- `./kit audit`: only the 4 leaks already on HEAD (atv_wheel.rs, plan/DONE.md).
- Original rides: no ride code changed (one new module, one `pub mod` line), so every replay hash
  stays.

## Not verified / not 1:1

- **The listeners are the original's, not the port's.** The first frame matches row 0 with the
  world step and the body's integrate in Rust. The bike's PreProcess, Force and PostProcess (ground
  head, ground step with the wheels' suspension and tire forces, air state, vehicle post) and the
  other five preforce listeners still run as SLUS-21104's code. Porting them is F31b, and the
  ground they query is F31c. The hybrid frame is the harness F31b can swap them into one at a time.
  So "the seeded Ride's first frame equals row 0's node and wheels" holds only with the original's
  listeners writing them.
- **No `ride::Ride` is seeded** (see the decision above). Nothing yet rides a `Seed` on
  `Handling::MxVsAtv` in the game.
- **Params are read from the image, not from the bike's files.** The bike's `.phy` (`mx_*.phy` inside
  the disc's archives) is not extracted on this disk (openmx-mods holds only `Env/`). The tuning the
  seed carries (springs, damping, engine tables, body limits) is what the game parsed into the image,
  word for word. A check that `handling::mxvatv_phy` on the bike's own file gives the same words was not
  made.
- **The seed reads the body, the wheels' suspension and steering, and the engine.** It does not read
  the bike object's own words (levers, air, backflip, limiter state; the quad's reader has them at the
  same offsets), the wheels' tire tables, spin or contact frame (`atv_wheel::Spin`/`More`), the scene
  node or the collision record. F31b will need them, and `tests/atv_quad/image.rs`'s `read_ride` is
  the map to copy them from.
- Only state 4 and only one frame were tried.
