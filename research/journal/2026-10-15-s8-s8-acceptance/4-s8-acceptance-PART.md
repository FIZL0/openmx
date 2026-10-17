# 4 — s8 acceptance (PART)

Task S8. 2026-10-17.

The run ran. Three sessions built the recipe, the reader and the recording; P5 gave the harness a bike to
compare against and left two questions behind — where the capture is, and what the gate does when the port
disagrees. Both are answered, and with them the acceptance run stopped comparing two spawns and started
comparing a simulation. It still does not pass, and this entry is mostly about exactly where it stops and
why that place is not the physics.

## What I did

**1. Found the capture.** `the_acceptance_run` looked in `context/capture/` and the three captures of the
recording live in `context/capture/acceptance/` (V16 moved them there so `./kit check` would be green about
unrelated work in a checkout that has the data), so the run was skipping silently in every worktree and
failing at tick 0 in the main one. Decided the subfolder is right and the reader is wrong: `acceptance_dirs()`
is now `[context/capture/acceptance/, context/capture/]`, looked at in that order by both the named lookup
(`acceptance_capture`, `the_acceptance_run`) and the scanner
(`every_acceptance_capture_is_the_original_at_a_step_the_port_can_match`). No file was moved by me or by a
test. The reasons for that direction, written where the function is:

* `context/capture/` is scanned by six tests that each pick up any CSV carrying their columns — the input
  decode, the pad tape, the whole-frame lag tests, the format reader. An acceptance capture is the one kind
  whose verdict is *the status of the port* rather than a pass or a fail, so it does not belong in a
  directory where unrelated tests read it.
* `context/capture/` is still read, because that is where `./kit record -o` writes by default and a capture
  dropped straight out of the recorder should be found, not skipped.
* A test now asserts both places and that the capture in hand is in one of them — the thing that would have
  caught this.

**2. Gave the run its start state** (`acceptance_start`, and `Fixture::start` is now `Some` for every
acceptance fixture). This is the verdict P5 left: `tick 0: bike.pos[0] 4309.563 (0x4586ac81) vs 0`. The
capture is the game's ticks 2774..4584 of a session already minutes old and `Fixture::initial()` fell back to
`Sim::new(seed)` at `stock::SPAWN`, so the first comparison in the file was one spawn against another and said
nothing whatever about the port. What the capture carries of a start state is four quads, and all four are
now seeded from the original's own bits:

* the bike's **whole world transform** — basis in rows 0..2, origin in row 3. The basis goes in as a
  quaternion through `math::mat4_to_quat`, *not* through `Ride::place`'s `basis(at, forward, UP)`, because
  that rebuilds an up-righted frame from a forward vector and the bike in the first row is already rolled
  (its up axis is 0.0264, 0.9995, 0.0150). An up-righted copy of a leaning bike is a different state, and the
  test for this writes a 17-degree lean into a synthetic capture to prove the difference survives.
* the body's **velocity** quad, `w` included. The original's `bike_vel.w` is 1.0.
* `chassis.speed`, which is a pure function of the velocity and is read by the ground step on the frame
  before the integrator first writes one: seeded with the integrator's own four instructions
  (`mula`/`madda`/`madd`/`sqrt`) rather than with a host `sqrt`.
* `place()` is still called first, for its clean slate (the derived fields, the wheels, the air state and the
  ground step's accumulators zeroed together), and then the measured pose goes in over the top.

Everything else about that instant is not in the capture: the engine's revs and gear, the suspension's
compression, the wheels' spin, the air and trick state, the balance accumulators, the input layer's press
timers, the area's respawn point. All listed below.

**3. `replay::record_from`** (new, `crates/game-sim/src/replay.rs`): `record` from a state somebody else
built rather than from a seed, returning it in `Fixture::start`. `record` itself is untouched and still
returns `start: None`, so no regression fixture changed. The synthetic-capture generator needs it: since P5
the bike columns of a synthetic capture are the port's own run of that capture's inputs, and since today the
*start* is also read from the rows, so the two passes have to be a fixed point — the blank pass's start is the
origin with an identity basis and no velocity, a run from there puts exactly that in row 0, and reading row 0
back gives the same start again. Recording from `Sim::new`'s spawn instead would have made every synthetic
acceptance fixture diverge at tick 0 against its own reader.

**4. Decided the gate: it reports, down to a recorded tick** (`ACCEPTANCE_AGREES_THROUGH = 0`,
`acceptance_verdict`). P5 left this choice to S8 explicitly. A divergence in the acceptance run is the status
of the port and not a regression in whoever ran the tests: the world is not in a fixture, the bike is a
stand-in and two movement tasks are open. A gate that panics on that reddens `./kit check` for every
unrelated task in any checkout that holds the data — which is exactly how the captures came to be parked out
of the readers' way — and a gate that is merely silent proves nothing. So it is a ratchet:

* agreement on every tick → passes, loudly;
* a divergence *after* the agreed tick → reports how far it got, plus the first tick each of the eight lanes
  parts company on (A1's field-level diff, one replay per lane), and asks for the constant to be raised when
  the run gets further;
* a divergence *at or before* it → panics;
* `NoSuchField` → reports; that is the fixture's fault, not the port's.

The floor is 0 and 0 is not nothing — it is the whole of item 2, and it bites: a test strips `start` from a
fixture (what every acceptance fixture was until today) and the gate fails with "gone backwards".

**5. Added S28** for the ground, because ticking S8 while the run does not pass would otherwise leave the
plan claiming an acceptance run the port does not have.

## What the original does

Nothing decompiled this session. The original's side of this entry is the capture: `watch/acceptance.toml`
read off a running game through PINE, 1,811 rows of the human's own minute (`first_recording.p2m2` played back
over the accessibility bus in session 3), 54 columns, frames 2774..4584 of the game's own `frame_counter`.
What that session established about it — the step is the 30 fps clamp on every tick, there is one row per
tick with no gap, the actor is the local human's and not one of the area's three AI riders, and the game reads
the second of a tick's two video fields — is unchanged and is what makes the rows a fixture at all.

One new reading of the original's data, from row 0: the bike's velocity quad carries **1.0 in `w`**, and its
transform's origin carries 1.0 in `w` too. Both are now seeded, and both agree with the port on all 1,810
recorded ticks (below), which is the first thing in this project to be proved 1:1 against the original over a
whole minute of real play.

## How it was proved

**The acceptance run itself**, `./kit check` green (71 tests in `--test replay`), on the freeride
capture in `context/capture/acceptance/`: 1,810 frames, 14,488 checks, seed 0xd7f1ddd0.

```
the acceptance run of acceptance-freeride (1810 ticks of real play, rng 0xb646bb20d7f1ddd0,
dt 0.03333..0.03333) reaches tick 1 of 1810 before it parts from the original:
tick 1: bike.pos[1] 208.02884 (0x43500762) vs 207.98192 (0x434ffb5f)
  bike.pos[0]: agrees through tick 29, parts at 30
  bike.pos[1]: agrees through tick 0, parts at 1
  bike.pos[2]: agrees through tick 1, parts at 2
  bike.pos[3]: agrees on every recorded tick
  bike.vel[0]: agrees through tick 0, parts at 1
  bike.vel[1]: agrees through tick 0, parts at 1
  bike.vel[2]: agrees through tick 0, parts at 1
  bike.vel[3]: agrees on every recorded tick
```

So: **tick 0 agrees on all eight lanes**, bit for bit, where it was a 4,309-unit teleport this morning. The
two `w` lanes agree on all 1,810 ticks, through both crashes. `bike.pos[0]` agrees for 30 ticks — the bike's
x velocity in the first row is 4.7e-05, so a second of x position is identical in the last bit, which is
weak evidence about the physics and good evidence that the seeding and the tick alignment are right.

**Why tick 1 parts, measured and not guessed.** Over the first eight ticks the port's bike loses 0.0475,
0.0809, 0.1165, 0.1519, 0.1872, 0.2222, 0.2569 units a tick. Second differences are a flat 0.0353, which at
the 30 Hz step is an acceleration of 31.8 units/s² — `phy::G` (32.2) and nothing else. The original's bike
loses 0.00058 a tick, steadily, rolling down a gentle slope. **The port's bike is in free fall.**
`replay::verify` replays over `world::Flat` at y = 0 and the recording is a ride over Free09 at a ground
height of about 208, so the port's wheels find nothing 208 units below them while the original's are on a
hill. A fixture has a start state now and still has no ground. That is S28, and until it lands there is no
tick 1 to agree about — the physics has not been tested by this run yet, in either direction.

The second capture, the scripted 900-tick ride from the same state, says the same thing in the same words:
tick 0 agrees on eight lanes, tick 1 parts on `bike.pos[1]` at 208.01945 vs 207.97247, and its `pos[0]` holds
for 89 ticks. The third predates the `bike_ctl_src` column and the scanner skips it.

**The new code, proved on synthetic captures** (`--test replay`, 71 tests):

* `a_fixture_starts_where_the_capture_starts` — three ways, because each is a different way to get it wrong:
  all eight lanes of `Fixture::initial()` are the capture's first row bit for bit; the whole basis survives a
  17-degree lean (the up axis is within 1e-5 of the capture's, where an up-righted rebuild would be 0.29
  away — five orders of magnitude more than the quaternion round trip); and the start is not `Sim::new`'s
  spawn, so a fixture that quietly lost its start state cannot pass.
* `the_acceptance_gate_fails_when_a_run_stops_starting_where_its_capture_does` — `start` set to `None`,
  `#[should_panic(expected = "gone backwards")]`. Shown to fire: `tick 0: bike.pos[1] 0 vs 1`.
* `an_acceptance_capture_is_looked_for_where_it_is_kept_and_where_the_recorder_writes_it` — both directories,
  in order, and the capture in hand is in one of them.
* Everything session 2 and 3 built still passes unchanged on the synthetic captures, which now carry a start
  state too: the reader end to end, the five refusals, the half-stamp trap, the gap refusal, the
  spec-and-reader agreement.

## Not verified / not 1:1

- **The acceptance run does not pass, and the physics in it is untested.** It parts from the original at tick
  1 of 1,810. The cause is measured (free fall against Free09's ground, above) and is the harness's missing
  world, not a ported function, so this run says nothing yet about whether S3's movement or S10's air are
  right — not that they are wrong either. **S28** is the ground; until it lands, `ACCEPTANCE_AGREES_THROUGH`
  is 0 and the only thing the run proves about the simulation is its start state.
- **The bike is the stand-in 250 and the recording is a Race125** (`stock::BIKE`, no number in it the
  game's). Even over the right ground the suspension and engine forces cannot match. That is **D7**, and it
  is why raising the floor will need D7 and S28 together rather than either alone.
- **The start state is four quads out of a state with hundreds of words in it.** Not seeded, because the
  capture does not carry them: the engine's revs and gear, both suspensions' compression and last
  compression, both wheels' spin, the air state, the trick machine's clocks, the ground step's balance and
  pitch accumulators, the peak-g history, the four `Deriv` histories, and the input layer's press/release
  timers and event queue. Each is a real difference from the original's state at that instant. `place()`
  zeroes them, which is a bike just put down rather than one mid-ride; a capture whose watch chain reached
  them, or a save-state RAM image of the vehicle struct at that tick, would close it.
- **`chassis.local_vel` is left at zero** rather than the velocity rotated into the body frame. The ground
  step writes it every frame from `mv::Step`, so it is an output on every tick but the first; on tick 1 the
  port reads a zero where the original had a real number, if anything reads it before it is written. Not
  chased because the run cannot get past the ground anyway.
- **The respawn point is where the capture starts.** `Ride::spawn`/`spawn_facing` are seeded from row 0,
  which is not the area's start line — the capture does not say where that is. It only matters if a respawn
  fires inside the window, and the recording's first wreck is at tick 485 of 1,810 (S14's measurement, ticks
  3259..3349 in `frame_counter` terms), so it will matter before the run is finished. The alternative,
  leaving `stock::SPAWN`, would teleport the bike 4,300 units on a respawn; both are wrong and this one is
  bounded.
- **The text fixture format still cannot carry a start state.** `Fixture::start` is `Option<Sim>` and
  `Fixture::parse`/`to_text` do not write it, so a `.fixture` file in `context/fixtures/slow/` — the form
  `the_acceptance_run` prefers if it exists — would replay from the spawn and trip the ratchet at tick 0.
  Not fixed (it needs a reader and writer for the whole state, which is a task of its own); instead the
  capture is documented as the canonical form, the gate's own failure message says so in as many words, and
  nobody has written such a file.
- **`ACCEPTANCE_AGREES_THROUGH` is a hand-maintained constant.** Nothing makes a session raise it, so a
  session that improves the port and does not notice leaves the gate loose. The verdict prints the number to
  raise it to when the run gets further, which is the cheapest thing that could work and is not a guarantee.
- **The per-lane diff is eight independent replays, not one walk.** Each replays the whole fixture watching
  one lane, so its "agrees through tick N" is "this lane alone agrees through N" — a lane can be right while
  the state behind it is already wrong. That is the honest reading of a watch and not a defect, but it is not
  `hash::first_difference` over the whole state, which no acceptance capture can produce (the original's hash
  cannot be captured).
- **Nothing was read off the disc or the emulator this session.** No PINE, no new capture, no decompilation;
  the emulator copy was never started. The captures compared against were made in sessions 2 and 3 and copied
  into this worktree from the main checkout, because `context/` is per-worktree.
- **The third acceptance capture is not read by anything.** It predates the `bike_ctl_src` column the scanner
  filters on, so it is skipped silently rather than refused. Left alone: re-recording it is cheap and nothing
  needs it.

## Next

**S28, with D7 beside it.** Concretely, for whoever takes it:

1. `replay::verify_in(f: &Fixture, world: &mut dyn World)` beside `verify`, and the same for `state_at`;
   `Sim::frame` already takes a `&mut dyn World`, so the seam is only in the harness.
2. The capture has to say which world. `watch/acceptance.toml` records no track or area id; add one (the
   scene's own name, or the track index the session loaded) so a fixture can ask for the right terrain rather
   than being handed one by hand.
3. `game_sim::world::Ground` over `track::Terrain` is the port's own ground, and
   `crates/game-sim/tests/track.rs` already opens the disc's archive from `context/iso/` and skips cleanly
   when it is absent — that is the pattern an acceptance run should follow, so the test stays green on a
   machine with no disc.
4. The props and walls (`World::solid`) are a separate question; `Flat` answers false and Free09 has fences.
   The first eight ticks will not care.
5. Then raise `ACCEPTANCE_AGREES_THROUGH` as far as it goes and read the per-lane diff: if `pos[1]` holds and
   `vel[0]`/`vel[2]` part, that is friction and drive (S3); if everything parts at once, suspect the `.phy`
   (D7) before suspecting a ported function.

The captures are in `context/capture/acceptance/` and have to be copied from the main checkout into whichever
worktree runs this (`context/` is per-worktree, and the whole suite is green without them).
