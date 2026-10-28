# 1 — nat01 rain (FINAL)

Task V73. 2026-10-28.

## What I did

- **`crates/openmx/src/weather.rs`** (new, std + `game_render` only, so the test compiles it in by path):
  - `boxes` reads a `.toy`'s `[WeatherBoxToys]` / `[WeatherBoxToyN]` with every key and the original's
    default for each missing key;
  - `triggers` reads the `[TriggerN]`s that have an `ActionType = Weather` action. The volume and contact
    keys are read as V71's `particle_triggers` reads them, plus the action's `WeatherBoxName`,
    `ChangeDensity`, `ChangeDensityStep`, `NewAspectRatio` and `NewMin/MaxParticleVelocity`;
  - `Rain` is one box:
    - `touch`: V71's trigger contact. A trigger under 100 percent rolls once as a rider comes in;
    - `act`: the trigger action's apply;
    - `update`: the toy's update, which ramps the density;
    - `emit`: the toy's draw, which moves the box onto the camera, sets the slant, and restarts or cuts
      packets;
    - `drawn` and `glows`: each live drop as a streak quad turned to the eye.
- **The shell** (`main.rs`):
  - `load_rain` reads the track's `.toy` and the box's `Material` `.pst` (`rain_gpu.pst`). It pushes the
    texture onto the glow list after the tire tread, so the GPU and the software path both get it;
  - on every simulated frame: `touch` with every seat's ride position, `update(dt)`, then `emit` at the
    tick camera of the box's `CameraIndex`;
  - every view gets one additive glow of the live drops, drawn `(1 − blend) × dt` back so the drops move
    smoothly between ticks at the display rate;
  - a world mod has none (tile feet, the same rule as V41/V45).
- **What happens on nat01** (the V41/V45 audits of every `.toy` found a weather box only here):
  - it starts dry (`StartDensity=0`);
  - `RainBegins`, the gate at 2565,3017 (75%), sets the target to 1 at 0.01 a frame. The rain comes in
    over ~3.4 s at 30 Hz, to all 267 packets (801 drops) live;
  - `RainEnds`, at 3340,2778 (50%), takes it back to 0 the same way.
- **opentennis**: `hst-play/src/play/weather/rain.rs` draws its own streaks in a world-tiled box. It is a
  Bevy port of a different original's rain, so I took only the idea of drawing each drop at the frame's
  own time, not the code.

## What the original does

Addresses are in `research/oracle.txt` § V73. I read the PC exe's `WeatherBoxToy` / `WeatherTriggerAction`
first (`./kit fn 0x0060b600 -o pc` and its neighbours). They have the same fields at +8, so the PS2
functions were easy to find from the key strings.

- **The read** (`./kit fn 0x003d9cb0`) gives every key above its default. It draws two 16-entry tables
  from the C `rand`: extra life `(MaxLife − MinLife)·r`, and a start depth used only with
  `AdjustLifeAndRangeToSpeed`. It also builds a 16-entry size table, `MinSize + i·(Max − Min)/15`, and
  `3 × NumberOfParticles` start points uniform in the box. A point's height is
  `Box.y/2 − Box.y·MinRange%·r`, which is the box's top on nat01. Packets: `n/3 + 1`, of 3 drops each.
- **Speeds** (`./kit fn 0x003da550`): `v[i] = Min + (15 − i)(Max − Min)/15`.
- **Update** (`./kit fn 0x003daa70`): a three-state ramp. The step is per frame, not scaled by dt. Each
  turn costs a frame. While the density rises, life is halved. Ported exactly in `Rain::update`, and
  `the_ramp_turns_a_frame_late` pins the sequence.
- **Draw** (`./kit fn 0x003dac10`, `0x003db130`):
  - the box centre is the camera's position;
  - the slant `dir = norm(centre + (0, CameraTilt, 0) − last centre)`;
  - a packet whose time is up restarts while `live < int(n·density)`: `time = clock + life + extra[idx]`,
    and 3 drops of `velocity (−dir.x, v[idx], −dir.z)`, `size[idx]`, a streak `size·aspect` long along
    `dir`, from `centre + point[k]`;
  - then every live packet at an index ≥ the live count is cut.
  - The VU1 microprogram (`Vu1RainSimulation.vuo`) moves and draws the drops. I did not read it.
- **Action** (`./kit fn 0x003db500` reads it, `0x003db600` fires it): it finds the `ToyWeatherBox` by
  name. Density ≥ 0 sets the target, step ≥ 0 sets the step, aspect > 0 sets the aspect, and either
  speed > 0 sets both speeds and rebuilds their table. The trigger's contact is the one V71 ported.
- **Material** `rain_gpu.psm`: blend A=Cs, B=0, C=As, D=Cd, which is `Cs·As + Cd`, additive. The port
  draws it additive.

## How it was proved

- **The disc.** `crates/openmx/tests/weather.rs`:
  - `nat01s_weather_box_and_triggers` checks every key of nat01's box, that `rain_gpu.pst` parses, and
    both triggers' names, percents, actions and 2.5 ft thickness;
  - `rain_begins_and_ends`:
    - dry for 60 frames;
    - a rider standing in `RainBegins` rolls once and not again;
    - riding through it brings the density to 1 in 99–103 frames, with life halved on the way;
    - 267 packets live and over 700 drops, every one inside the box's 90×125 footprint and its fall;
    - a moved camera leans the streaks at 45° for a 4 ft move against the 4 ft tilt;
    - `RainEnds` puts every drop out;
  - `the_ramp_turns_a_frame_late` checks the state sequence up and down, and the speed-table rebuild.
  - `./kit check -- -p openmx --test weather` passes 3 tests on this slot, which has the disc.
- **By eye.** `openmx --headless --track nat01 --trace-at 2564.8,3016.85,180 --shot … --shot-frame 400`
  shows grey-white streaks over the track, on both the GPU and the software renderer.
- **Not compared against the original's picture** (below).

## Not verified / not 1:1

- **No picture of the original's rain.** Every save state is on free09 (plus the biplane's free06), and
  getting the original onto nat01 means driving its menus by pad. The look (tint 1.0, streak width =
  size, length behind the head, quad facing the eye) is the port's guess at what the VU1 microprogram
  draws. I did not read the microprogram.
- **The random numbers are the box's own xorshift, not the game's `rand`.** That covers the trigger
  roll, the life and depth tables and the start points. The original's read makes 7,232 C `rand` draws
  at nat01's load (32 for the tables plus 3 × 2,400 for the points), and each roll makes one more. The
  port's simulation `Rng` does not advance for them. A future bit-exact nat01 replay would need those
  draws added to the sim's stream at load and on each roll. Free09's replays are unaffected: it has no
  weather box.
- **The trigger contact is V71's approximation**: the rider's ride position in the box, and the
  last-frame contact list per trigger. `IgnoreTrigger=AIANIMATION` is not read, since the port has no AI.
- **Which camera**: the box follows the tick camera of `CameraIndex` (0), as the original does. In a
  split session the other views see camera 0's rain, so player 2 rides dry when far from player 1. That
  is faithful, but I did not check the original's split screen.
- **The update's gates are not ported**: the toy's enabled flag (+0x3c) and the pause byte. The port
  only steps it on simulated frames, which leaves the rain frozen in a pause.
- **`AdjustLifeAndRangeToSpeed`, `MaximumSpeed` and `Offset` are not run.** nat01 has the first off and
  sets neither of the others, and the V45 audit found no other weather box.
- **The camera axis the width is crossed with** (camera +0x150) is replaced by facing each view's eye.
- **No rain sound**: the `.toy` has no `[ToySound_*]` for it, and I did not look for one elsewhere.
- **No reset on a race restart**: `Rain::reset` exists, but nothing calls it. The shell's TrackFx is not
  reset either, and the port has no race restart.

## Next

Nothing for this task.

Landed with `--skip-audit`: `./kit audit` shows only the 7 leaks already in plan/DONE.md (other
tasks' addresses), none in this diff.
