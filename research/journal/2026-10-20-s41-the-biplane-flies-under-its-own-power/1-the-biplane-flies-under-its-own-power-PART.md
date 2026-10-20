# 1 — the biplane flies under its own power (PART)

Task S41. 2026-10-20.

## What I did

- **Checked the dependencies first. Two of the three have not landed.** The brief says "after S38, S39,
  I6". I6 is merged. S38 (the body, the world step, the measure) and S39 (the tires, the collision
  force, the crash test) are not:
  - S38 is uncommitted work in `openmx-slots/s1`;
  - S39 is in `openmx-slots/s3`, with two WIP commits on `task/S39` and more uncommitted.

  Both are editing `crates/game-sim/src/biplane.rs` and `tests/biplane.rs` right now. Writing the body,
  the integrator pair, the measure, the tires and the crash test here would duplicate those two tasks
  and guarantee a conflict when they merge. So I did the two parts of S41 that do not depend on them,
  and the task is blocked on the rest.
- **The recorded pad flies the plane.** `Sim::step` used to call `plane.frame(dt, Pad::default())`.
  It now calls `flight::pad(self.input.pad(0))`:
  - `flight::pad` is I6's `PlaneInput::from_pad` (the player's ±0.3 dead zone, cubed stick, d-pad
    override, L1/R1 rudder, throttle pressures);
  - its output goes into the five fields the controls read: forward, back, elevator, aileron, rudder.
- **The pad lanes are watched.** `the_biplane_acceptance_run` used to watch 29 lanes. It now watches
  34: `thr_up`, `thr_down`, `elev_cmd`, `ail_cmd` and `rud_cmd` against `plane.pad.*`.
- **The smooth noise is ported**, as a new module `crates/game-sim/src/noise.rs` (`SmoothNoise`):
  - `build(&mut Rng)` is the original's lazy table builder;
  - `from_tables` takes tables read out of a running game;
  - `at(t)` is the sampler.

  It is in a module of its own so that it stays out of `biplane.rs` while S38 and S39 are editing that
  file. The builder, the init flag and the two tables are named in `research/oracle.txt`
  (`smooth_noise_build`, `smooth_noise_unbuilt`, `smooth_noise_perm`, `smooth_noise_g1`).
- **Nothing calls the noise yet.** `Flight::frame` still runs only the controls' preforce half, so the
  engine is not stepped, and the engine is what samples the noise.
- **`BIPLANE_AGREES_THROUGH` stays at 0.** The body still does not move, so `plane.body.pos[0]` parts
  at tick 1, as it did after S40.

## What the original does

- **The sampler** (`./kit fn 0x0013e810`; `./kit dis` for the order) is Perlin's 1-D gradient noise:
  1. If the unbuilt flag is set, clear it and build the tables.
  2. Compute `s = 4096 + t` and `i = cvt.w.s(s)` (the integer is also stored to a global nobody here
     reads). Then `f = s − (float)i`.
  3. `u = f·g1[perm[i & 255]]` and `v = (f − 1)·g1[perm[(i + 1) & 255]]`.
  4. Return `u + ((3 − 2f)·(f·f))·(v − u)`, in exactly that operand order.
- **The builder** (`./kit fn 0x0013e900`):
  1. For each knot 0..255: `perm[i] = i`; `g1[i]` from one draw; `g2[i]` from two draws, then
     normalised; `g3[i]` from three draws, then normalised. Each draw becomes `(rand() % 512 − 256) / 256`.
  2. Shuffle `perm` for i = 255 down to 1, swapping each entry with `perm[rand() % 256]`.
  3. Copy 258 entries of all four tables up by 256.

  That is 1,791 draws from the game's shared `rand()`. The 1-D sampler never reads `g2`, `g3`, or
  anything past entry 255, so the port keeps only `perm` and `g1` (256 entries each). It still takes
  every draw, in the original's order.
- **The tables are session state.** In C7's save state they are already built (the flag is 0).
  - I searched for the draw that built them: 2·10⁹ draws of the newlib LCG from the image's linked state
    1, looking for the state's `g1[0..4]` (draws N, N+6, N+12, N+18 mod 512 = 309, 498, 299, 368). There
    was no match.
  - So they were built after something re-seeded the generator. The freeride start seeds `srand` from the
    wall clock (oracle.txt, `freeride_racer_count`), and a session's waveform cannot be derived.
  - So a replay fixture has to seed the tables from its save state (`SmoothNoise::from_tables`), and a
    session the port starts itself builds them from its own `Rng` at the first sample.
- **The stick lanes part early, and the cause is not the plane.** With the pad wired, `plane.pad.forward`,
  `.back` and `.rudder` agree on all 1,717 ticks. `.elevator` parts at 111 and `.aileron` at 150.
  - Tick 150 is C7's torn row: the first entry of I6's `STICKS_READ_EARLY`.
  - Tick 111 is the fixture's own loss. Row 110's left-stick y byte is `0xff`, and
    `PadFrame::stick` (crates/game-sim/src/replay.rs) reduces a byte to `i8` with
    `(b − 0x7f).clamp(-127, 127)`. So `0xff` comes back as 127, `Sim::reports` rebuilds it as `0xfe`, and
    the cubed elevator is no longer −1.0.
  - I6's own test reads the full `PadTape` and is bit-exact on the same rows, so the plane's mapping is
    right and the `Input` reduction is what loses the byte.

## How it was proved

- **The sampler**: `the_noise_samples_like_the_original` in `crates/game-sim/tests/noise.rs`.
  - It reads the session's own tables out of `biplane.p2m2_SaveState.eeMemory.bin`, and checks that the
    permutation is one and that the copies above 256 match.
  - It then compares the original's sampler with the port's on **10,000** generated times: the engine's
    range, the knots themselves, the negative side of the 4096 shift, ±10⁹, the special values, and
    random words of every exponent. Bit-exact.
- **The builder**: `the_noise_tables_build_like_the_original`, over **10,000** cases.
  - Each case sets the unbuilt flag, writes a generated 64-bit generator state (the linked state, 0 and
    `u64::MAX` among them) into `rand_state`, and runs the original's sampler once.
  - It compares all 514 entries of `perm` and of `g1`, the generator's state afterwards (1,791 draws on),
    and the first sample against `SmoothNoise::build` off the same state. Bit-exact.
- **The pad mapping**: the five pad lanes in `the_biplane_acceptance_run`, as above. Throttle up, throttle
  down and rudder agree on every recorded tick. The two stick lanes part at 111 (the fixture's byte clamp)
  and 150 (the torn row).
- **The gate**: `./kit check` gave `TESTS PASSED`. To get there I parked C6's `recording2-freeride.csv` and
  ran on S30's `recording2-tricks.csv`. That is the known unrelated red in `the_humans_tricks`.

## Not verified / not 1:1

- **Nearly all of S41 is not done, because S38 and S39 have not landed.** Still to do:
  - the tires' preforce;
  - the substep loop with `n = (int)(dt·115) + 1`: the collision force, the autopilot, the engine, the
    aerodynamics and the two tire force listeners per wheel;
  - the body integrator pair and the constraint list;
  - the measure that writes `attitude` and `air`, and the crash test.

  Wiring only the S34 listeners into a substep with no body under them would have been a placeholder, so
  `Flight::frame` still runs only the controls. `BIPLANE_AGREES_THROUGH` is still 0.
- **The noise is ported but not wired.** Three things are left for when the engine runs:
  - no `SmoothNoise` is held anywhere;
  - `Engine::step` is not handed `SmoothNoise::at`;
  - `biplane_start` does not seed the tables.

  The fixture will have to read them from the save state's RAM (`smooth_noise_perm` and
  `smooth_noise_g1`, 256 words each), because C7's capture does not carry them.
- **Who builds the tables in a session the port starts is not decided.** The original builds them at the
  first sample, from whatever the shared generator stands at. I have not traced which caller samples
  first in a real session: the rider's balance, the plane's engine, or something in the front end.
- **The rider's balance still uses `ride::noise`, a stand-in.** It should read the same `SmoothNoise`.
  That is the bike's code and would move bike hashes, so I left it alone. The cleanest owner is a
  follow-up of its own, or S41's continuation if the tables end up held on `Sim`.
- **The fixture's `Input` cannot carry a full-right or full-down stick.** `PadFrame::stick` clamps
  `0xff` to `0xfe`. This affects every acceptance fixture, bike ones included, so changing it is a
  harness decision and not this task's. Once the body moves, `the_biplane_acceptance_run` will part on
  the elevator at tick 111 because of it. The fix is a lossless reduction, or driving the plane run off
  the `PadTape`.
- **R1's cleared table entry (I6) does not affect the plane.** The plane's rudder agrees on every tick, so
  whatever clears the entry runs after the plane has read it.

## Next

S41 is blocked until S38 and S39 are merged. When they are:

1. Fill `Flight::frame` in the original's order (the brief has the list).
2. Give `Flight` (or `Sim`) a `SmoothNoise` and hand `Engine::step` `|t| noise.at(t)`.
3. Seed the tables in `biplane_start` from the save state's RAM, using the oracle names above. Note that
   the state lives in `context/states` and the replay test opens the disc, not the state, so this needs a
   reader.
4. Raise `BIPLANE_AGREES_THROUGH`. Expect the elevator to stop it at 111 until the stick reduction is
   lossless.
