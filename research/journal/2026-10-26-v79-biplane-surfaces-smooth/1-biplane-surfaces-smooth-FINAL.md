# 1 — biplane surfaces smooth (FINAL)

Task V79. 2026-10-26.

## What I did

**What the jumps were.** The original eases its surfaces with one thing only: the rate limits in the
plane's control law. The port already has that, bit for bit (`game_sim::biplane::controls`, S34). The
rate limits on the disc's `biplane.phy` are:

| surface | rate limit | one 30 Hz tick | full throw (rest to end) |
|---|---|---|---|
| ailerons | 9 rad/s | 0.3 rad (17°) | 0.611 rad, two ticks and a bit |
| elevator | 9 a second (fraction of `MaxElevator`) | 0.3 | four ticks |
| rudder | 1 rad/s | 0.033 rad | 0.349 rad, eleven ticks |

The original draws each 30 Hz tick as it is, so its ailerons step 17° a tick. The shell draws above
the tick rate. Since P52 it has slid the plane's placement and the camera between the last two ticks,
but V38 posed the surfaces from the last tick alone. So the plane glided while its surfaces held still
for a tick and then stepped. That is the jump the user saw. The brief's fallback is what fixes it:
blend the surface poses between the two ticks, the same way the placement is blended. The control law
already eases the surfaces toward the command, so the eased half of the fallback was already there.

- `game-render/src/biplane.rs`: `between(was, now, t)` makes one drawn frame's `Surfaces` from two
  ticks'.
  - Each angle goes straight from `was` to `now`, which is the control law's own ramp cut finer.
  - The prop takes the short way round its wrap. The human's flight turns it 0.76 rad a tick, well
    under half a turn.
  - A wrecked prop and the cone are taken from `now`, because a crash is a switch and not a move.
- `openmx/src/plane_draw.rs`:
  - `shown(was, plane, t)` gives the surfaces a frame draws: blended, or `now` when there is no `was`.
  - `actors_with` and `skinned_with` pose the plane by surfaces handed in. `actors` and `skinned` stay
    as they were, as wrappers, for the bays' and the markers' planes, which do not move.
- `openmx/src/main.rs`: `before_surfaces` holds each seat's plane surfaces from before the tick, next
  to `before_planes`. The flown planes' GPU and software draws blend from it by `blend.0`. A cut frame
  (P120) draws `now`, the same as the placement.

opentennis has no vehicles with moving parts, so there was nothing there to take.

## What the original does

- **The ease is the control law's rate limit.** `toward(value, stick × max, rate × dt)`, once a tick,
  for each surface (S34's port of the preforce listener; the `[Joystick]` keys are in R8's
  `biplane_read_phy` block in `research/oracle.txt`). There is no other filter on the bones: the
  surface step (`./kit fn 352ff0`, S35) writes the angles it is handed straight onto them.
- **It does not interpolate between ticks.** In a PCSX2 Video Capture of the biplane state with the
  stick thrown, 323 of the 325 picture changes in 667 frames come with exactly one still frame between
  them. Each 30 Hz tick is shown as two identical 60 Hz frames, the same as V53 found for the HUD.

## How it was proved

- **The original's curve, every row of a PINE recording.**
  `game-sim/tests/biplane_anim.rs::the_originals_surfaces_ramp_at_the_control_laws_rates` reads S35's
  `pilot.csv`, the human flying the biplane (1718 rows). The `[Joystick]` limits come from the disc's
  `biplane.phy`.
  - The aileron: every row's angle is the port's `controls` run on the row before's angle and stick.
    It matches bit for bit on all rows but the capture's three known `TORN` rows: over 1700 rows, 61
    of them a full rate-limited step.
  - The elevator and the rudder: no row steps further than its limit (bounds only; see below).
  - I proved the test ran on the data by forcing its count assertion red once.
- **A stick step on the port follows that curve.**
  `game-sim/tests/biplane_anim.rs::a_stick_step_moves_each_surface_along_the_originals_curve` uses the
  disc's limits. The stick and the rudder go full one way, then the other, then centre, 30 ticks each.
  - Every tick, each surface steps exactly its rate toward the stick, or less only on its last step.
  - None of the posed angles (flap, rear flap, rudder) moves further than a tick's rate.
  - The rudder takes 11 ticks to get there, and the surfaces then hold.
- **The drawn frames glide.**
  - `game-render`'s `frames_between_ticks_glide_along_the_ramp` draws a two-tick ramp four frames a
    tick. Every frame moves each surface a quarter of the tick's step. The frame on a tick is that
    tick. The prop crosses its wrap forward, and a crash switches the cone at once.
  - `openmx/tests/biplane_surfaces.rs::a_stick_step_glides_between_the_drawn_frames` flies the disc's
    plane through the shell's `Garage`. It throws the stick up-left with R1 for 20 ticks and draws four
    frames a tick through `plane_draw::shown`. No frame moves a surface more than a quarter of a
    tick's rate, the frame on each tick is that tick's pose, and all three surfaces get over: aileron
    0.61, rear flap 0.52, rudder 0.32. I proved it ran on the disc by forcing an assertion red once.
- **Video Capture** (`capture/film.sh`, `capture/pairs.py`, `capture/sheet.py`; film in this slot's
  `context/v79/throw.mp4`, not committed): the per-tick pairs above. The stick throw rolls the plane
  over about ten ticks.

## Not verified / not 1:1

- **The aileron angle was not read off the film.** At the chase camera's distance the flaps' trailing
  edges are a few pixels thick, and the camera rolls with the plane, so I could not measure an angle per
  frame from the picture. The per-tick angles are proved from the PINE rows instead. The film proves
  only that the original draws once per tick.
- **The elevator's and the rudder's curves against the original are bounds, not bits.** `pilot.csv`
  has neither the roll the elevator fades by nor the L1/R1 buttons behind the rudder. Every captured
  step is within the limit, but they were not replayed bit for bit. The elevator's fade and the
  rudder's law are S34's port, proved there.
- **The port draws in-between poses that the original never shows.** This is a deliberate rendering
  choice, per AGENT.md's "interpolating between simulation ticks on the render side only". The frames
  on a tick are the original's pose. The simulation and the replay hashes are untouched.
- **The bays' and the markers' planes are not blended.** They are drawn parked or as ghosts, with their
  surfaces at rest, so there is nothing to blend.
- **The pilot's lean was not looked at.** It is a separate path (V39's palette off the body's lags).
  The brief names only the control surfaces.
- **The emulator film was made from the biplane save state, with wall-clock pad timing.** It is not a
  tape, so the throw's tick is known only roughly (around frame 240).

## Next

Done. The first `./kit done` went red only on `openmx --test lobby` ("both seated: not in 30 s", six
tests, and again on its lone rerun) at a load average near 40. Bare HEAD, in a scratch worktree, then
passed all 48 lobby tests at load 20, and so did this branch a minute later. That is the known LAN load
flake, and these tests never draw a plane. The second try's lobby reds passed on the gate's lone rerun.
That try went red instead on `machines_everywhere`'s
`two_online_machines_start_their_seats_in_the_plane_on_a_race_track`, because a shell could not bind
its port ("127.0.0.1:47613: Address already in use", another slot's run). All 87 of that file's tests
passed on a rerun alone. The third try went red only on `one_fps`'s
`a_headless_ride_at_the_default_frame_keeps_to_its_budget`: a 69 ms frame at load average 44, on a bike
ride with no plane. Both of its tests passed alone a minute later.
