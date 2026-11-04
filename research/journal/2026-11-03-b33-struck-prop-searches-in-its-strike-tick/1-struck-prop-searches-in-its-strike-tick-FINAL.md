# 1 — a struck prop searches in its strike tick (FINAL)

Task B33. 2026-11-03.

## What the original does

The bike is filed on the props' node ahead of the balls. The node's collide walk re-reads the
filed count at every step. When the bike's rays strike ball 2, the strike wakes it, which files it
at the end of the list, so the walk reaches the ball in the same pass and the ball's probe searches
then. f27a1's image 2903, which is the strike tick (knock.py's "woken" lags a tick), shows:

- ball 2's list: the kind-1 mirror of the bike's strike, then its probe's kind-0 entry on the
  bike's frame (`bikef.col`);
- the bike's list: its own kind-0 entry on the ball, then the kind-1 mirror of the ball's entry.

## The image before the strike

No f27a1 image before the strike was kept, and every PCSX2 copy's pre-strike slot had been
overwritten. PCSX2 from a save state is deterministic, so I ran the knock again in copy 1:
first_recording's state in slot 8, `aim.py 919513 8 2 40 25 9`, c6's `resume_if_paused.py`
(aim.py leaves the game paused, and knock.py then stops with "frame counter stuck"), then
`knock.py b33 --at 2902`. The new `--at` saves slot 3 at that frame instead of at the near
trigger. Its images 2903, 2904 and 2906 are word for word f27a1's. The result is image
`s195-f27a1-2902` in `context/states/`, which is git-ignored, with ball 2 asleep (enabled 0).
The run's .p2s and csv are in `context/s195/b33*`.

The kit's PCSX2 config for slot s1 is `openmx-slots/pcsx2/s1`, not the `s1-slots` path that
slots.py names. I restored NominalScalar to 1.0 afterwards.

## What I did

- `Strike::by`: `prop_contact::strike` names the striking bike (its player, and where the props
  start among the bodies).
- `PropSet::strike_in(strike, dt, world, movers)` replaces the strike's step for a riding bike.
  `strike_then` (the old `PropSet::strike` body) wakes and chains as before. For each object the
  strike files, it now runs `search_late`: the object's search in the pass's world, its movers
  added to the pass's `PropMovers` by its set's near-set key (the new `PropMovers::keys`). It does
  not just hear the object any more (B28). Then the late object's probe mirrors on this bike (B29's
  `heard.1.probed`) go on `strike.contacts`, naming the object's place in `strike.struck`, up to the
  bike's capacity. `PropSet::strike` (no `by`) keeps B28's hearing.
- `search_late` is the pass's own late search, which `prop_props::strike_props` already used for
  objects that other props' rays file. It is now a method shared by both.
- Callers: `Sim::step` and `step_all` (`strike_into`), `step_bumped`, and `step_seat`, which uses
  the shared set's movers through the new `Shared::movers_mut` once a seat has run the pass.
- The wreck bike's rays were already same-pass: they wake before the props' search.

## How it was checked

- `tests/knock_launch.rs the_strike_tick_is_the_originals`: image 2902, one `Sim::step`, against
  image 2903. Before the change the ball left at 13.2 ft/s against 16.67, and the lists and the
  bike's velocity were off. Now the following are bit for bit:
  - ball velocity (16.665989, 11.392712, 18.652424) and position;
  - ball 2's whole list, every word;
  - the bike's list up to its arms (a second seeded ride up to `strike_in`);
  - the bike's chassis velocity (11.394951, 2.1953306, 15.020116).
- The other knock_launch tests still pass: 2903→2904 (B29), 2904→2905, 2905→2906, and the 60-tick
  ride against f27a1.csv. B28's stand-in (`a_struck_ball_is_heard_in_its_strike_tick`) now
  asserts that the probe's entry is filed behind the strike's mirrors and that it moves the ball.
  It used to assert that the hearing moved nothing, which is no longer true.
- `./kit check -- -p game-sim`: 1203 passed.
- `cargo test --release -p openmx -- prop knock ball seat shared`: all passed.
- `cargo check --workspace --tests`: no errors.

## Not verified / not 1:1

- A late object's rays against the other props (`prop_props`) are not cast in the same pass. Only
  its own probe (terrain, scene, bike frames) is. The original's walk would cast them too.
- The late search runs without the wreck bikes' hulls (`&[]`, no wreck strikes). With a downed
  rider beside a struck prop, its entry on the wreck bike waits a tick.
- The plane's strike (`flight.rs`) and the Kiowa's still use `PropSet::strike`, which is heard only
  (B28).
- When one strike files several props, the bike's list takes each late prop's mirrors in strike
  order. Only one prop has been checked against the original.
- Only one knock (f27a1, ball 2, free09) is checked, and it is from a single strike tick.
