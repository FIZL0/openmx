# 1 — the ball's push on the wreck bike, the wreck's rays on the ball, the Sim's wiring (FINAL)

Task S244. 2026-11-02.

## What I did

- Capture (PCSX2 copy 3, scripts in research/s244, states in s3's `context/s244/` and
  `context/states/s244-*`, rows in `context/capture/s244/`):
  - hit3: s197b `put_ball.py PID 3 2 4 0.3 2` from drop2-2804, then `strike.py hit3 2 --frames 40`.
    `strike.py` saves a state on each of the first eight ticks. Kept: hit3-2829..2836.
  - hit5: `throw_ball.py PID s244/hit3-put 4 -14 2.5 -55 2` puts ball 4 14 ft behind the wreck,
    2.5 ft up, and throws it at 55 ft/s along the wreck's way. Then `strike.py hit5 2 --frames 40`.
    Kept: hit5-2851..2858.
  - `ram.py NAME...` writes the RAM images.
  - Tried and dropped: hit4, where balls 1 and 2 also struck each other (prop on prop is not ported).
    Two other hit5 throws failed: one from -9 ft hit before the saves began, and one at 0.3 ft up
    bounced off the ground.
- Found (oracle S244 block):
  - The wreck bike's ray probe has an empty category list, so it strikes the balls' f.col. S197b's 0x8
    leave-out list is the sweep probe's.
  - So on a tick the wreck meets a ball, the wreck's list holds kind-0 entries naming the ball after its
    scene entries, and their mirrors lead the ball's list.
  - In hit3 2832 the bodies are separating: spring and friction only, no impulse.
  - In hit5 2855 the ball's first-substep impulse on its own entries goes into the wreck body in the
    props' step.
- Port:
  - `prop_wreck::wreck_rays_on_props` runs the wreck's ray tree against every prop mesh, with a probe
    like `prop_contact`'s `Strikes`. It returns the wreck's entries and each struck prop with its
    mirrors.
  - `strikes` = those rays → `PropSet::strike` (the mirrors onto the props' lists) → `struck_by_props`,
    the two joined by prop.
  - `WreckStrike.entries` is new. `WreckBike::tick_struck` files the entries ahead of the props'
    mirrors. `tick` now keeps the pushed bodies.
  - `Sim::wreck_pushed_back(i)` hands them back to the set. It runs in every step path that runs
    `wreck_strike` (`step`, `step_all`, `step_bumped`, `step_among`), before the props' step.
- Tests (tests/prop_wreck.rs, states + disc):
  - `the_wreck_bikes_rays_strike_the_ball`: hit3 2829..2835 bit for bit (both lists, both bodies),
    entries from 2832. 2832 without the wreck's rays fails (ball list 0 where the state has 4, wreck
    list 1 where it has 5, both bodies off).
  - `the_balls_impulse_moves_the_wreck_bike`: hit5 2851..2857 bit for bit. The wreck body changes in
    the props' step at 2855.
  - `the_sim_wires_the_wreck_bikes_strikes`: hit2-2828, hit3-2832 and hit5-2855 through a one-player
    `Sim` with the rider down and the state's wreck bike, in the step paths' order:
    1. the props' search;
    2. `wreck_strike`, the same as `strikes`, with the wreck named `players()`;
    3. the ball's list, against the state;
    4. the wreck's frame;
    5. `wreck_pushed_back` (the strike is spent);
    6. `props_step`;
    7. the ball and wreck bodies, against the state.
    It fails with `wreck_pushed_back` left out.
  - `the_ball_strikes_the_wreck_bike` (hit2) still passes.
- Checked:
  - The game-sim test suite, all 148 binaries (lib and replay included), passes.
  - `cargo build -p openmx` builds.
  - tests/hash.rs pins Sim at 12112. `entries` is not walked and not on the wire, like the rest of
    `struck`.

## Not verified / not 1:1

- One contact per ray: the wreck's entries on props go after its scene entries, in a separate search.
  The original's one narrow pass would keep one per ray across both. It does not come up in hit3.
- The wreck's sweep probe against the props is not ported.
- Prop on prop (hit4's balls 1 and 2) is not ported.
- The Sim test seeds the props from the state and steps one tick at a time. It does not ride a whole
  wreck through `Sim::step`, because the recorder's first saved state comes 2–3 ticks after the put.
