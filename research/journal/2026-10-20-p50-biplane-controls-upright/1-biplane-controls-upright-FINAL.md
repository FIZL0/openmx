# 1 — biplane controls upright (FINAL)

Task P50. 2026-10-20.

## What I did

- `crates/openmx/src/pad.rs`: `PlaneScheme` (`New`, the default, and `Original`) and `Wiring::planes`, one
  per seat. `PlaneScheme::fly(report, inverted)` is the remap. Upside down under `New` it:
  - mirrors the left stick's y byte (`255 - b`);
  - swaps Up and Down;
  - swaps L1 and R1, each bit together with its pressure byte.

  Anything else passes through as it is, and so does every report under `Original` or the right way up.
  `Seats::plane(seat)` and `PlaneScheme::changes_camera` are new too. The second exists because the
  original layout's camera button is L1, which the swap hands the plane as R1 while the plane is upside
  down, so the camera reads it back off R1 there. Select, the new layout's camera, is never swapped.
- `crates/openmx/src/vehicle.rs`: `Plane::inverted(was)` and `Garage::inverted(was)`. The plane counts as
  upside down when its up vector points below the horizon. The answer has hysteresis: it only changes
  once up.y is `INVERTED_MARGIN` (0.2, about 11° past knife edge) past the horizon the other way. The
  test reads the plane after the last step, which is the attitude the player was looking at.
- `crates/openmx/src/main.rs`: each frame, each seat updates `Seat::inverted` and its report goes through
  `seats.plane(from).fly(...)` after P40's `Scheme::ride`. That happens before the poll, and it is the
  report the tape records. When a tape plays, `inverted` is still updated, but only for the camera
  check, because the tape already holds the remapped report. `--biplane-controls new|original|new,original,…`
  is parsed with `Scheme::parse_list`'s rules, prints a `biplane controls:` line, and is in the help.
- `crates/openmx/src/menu.rs`: `Controls::planes`, the `biplane = …` line in `controls.txt` (one word, or
  one per seat), and `Front::wiring` reads it unless the flag was given. On the Controls screen each seat's
  row now carries both settings, for example "1P controls  New  plane steady". Left and right step through
  the four combinations, and the biplane setting is the one that changes first. I did this because the
  screen has no room for a second row per seat: nine rows would run into the title and the pad lines
  below them.
- opentennis has nothing like this (it has no vehicle that flies), so nothing was taken from it. The
  pattern is P40's own: a remap of the report before the poll.

## What the original does

**It turns nothing round.** I read the whole path the stick takes to the plane. No step looks at the
attitude to change a sign:
- **I6's input step** (`./kit dis 0x351e20`) writes the elevator and aileron as `-sign(v)*|v³|` of the
  stick, the rudder as ±1 from the two shoulders, and lets the d-pad override. No attitude is read.
- **The control law** (`./kit dis 0x352160`, ported as `game_sim::biplane::controls`, S34) reads the
  roll for one thing only: `abs(+0x5d4) < 0.8` (the nose within about 53° of level) and `abs(roll)`
  against π/2 pick which side the elevator's knife-edge fade is mirrored about. The fade scales the
  elevator between `MinElevator` and 1. It never changes its sign.
- **The autopilot** (`game_sim::biplane::autopilot`) holds the roll to `MaxRollAngle` from level, or from
  inverted, for its coordinated-turn feed-forward. The stick's own sign goes in unchanged.

So what the user feels as "the controls flip" is not a flip in the code. Every surface is the plane's
own, but the follow camera keeps the world's up: `game_sim::camera`'s view up is the constant `UP`, and
the heading is the nose flattened onto the ground. Past knife edge, two controls therefore read the
other way round on the screen:
- **The elevator.** Climb pulls the nose toward the pilot's head, which upside down is toward the ground.
- **The rudder.** The pilot's left is the screen's right.

The ailerons do not change: a roll about the nose looks the same way round from behind, whichever way
up the plane is. So `New` turns exactly the elevator inputs (the stick's y, Up and Down) and the rudder
inputs (L1 and R1) round, and leaves the aileron inputs (the stick's x, Left and Right) alone.

R8 and S41's notes have nothing on this. R8 lists "the sign of Climb against stick up" as unchecked.

## How it was proved

- **Pad level**: `upside_down_the_new_biplane_controls_turn_the_elevator_and_rudder_round` (pad.rs) runs
  20,000 generated reports (random buttons, pressures and all four stick bytes) through the game's own
  poll and I6's `PlaneInput::from_pad`. It checks:
  - `Original` (upside down or not) and `New` the right way up return the report byte for byte. The
    plane's simulation therefore reads exactly what it read before P50.
  - Upside down under `New`, the throttles and the aileron are bit-identical and the rudder is exactly
    negated. Both shoulders held is the exception: that is left-wins either way round, and it is the
    plane's reset chord.
  - Upside down under `New`, the elevator comes out on the other side of centre, to within 1e-6 of the
    poll's rounding of `-1 + 2b/255`. Both Up and Down held is the same kind of exception.
  - Applying the remap twice gives back the original report.
- **Flight**: `upside_down_the_new_biplane_controls_fly_the_way_they_do_upright` (tests/ride.rs) flies the
  disc's `biplane.phy` through the shell's own order: attitude, then `fly`, then the poll and the garage
  pass. Thirty seconds of gas gets the plane up. Full right stick then rolls it until up.y < −0.75
  (158 frames). From the upright state and from the upside-down state, I flew a third of a second of
  each control and subtracted a third of a second of gas alone. Output:
  `pulled stick lifts the nose 0.0508 upright, 0.1009 new, -0.1048 original; left rudder turns -0.0721
  upright, -0.0813 new, 0.0811 original`.
  - Under `New`, both controls go the same way upside down as they do the right way up.
  - Under `Original`, both go the other way, which is the flip the user reported.
  - The right way up, the two schemes are bit-identical.
  - Upside down, the original's scheme leaves the plane field for field equal to the raw report's,
    for three different inputs.
- **Menu**: the settings test in `tests/menu.rs` saves and loads a seat flying the original's biplane.
  The Controls test steps 1P's row both ways through the four combinations, then saves and loads it.
- **Shell smoke**: `openmx --track free06 --vehicle biplane --headless --frames 30 --biplane-controls
  new,original` prints `biplane controls: new,original,original,…`, and `--biplane-controls sideways` is
  refused.
- `./kit check`: TESTS PASSED. One `-p openmx` run before that had reddened on netplay's
  `four_peers_and_eight_riders_on_a_bad_network`, a known flake. It passed when run alone.

## Not verified / not 1:1

- **The whole feature is not 1:1, on purpose.** `New` is the port's addition, asked for by the user, and
  it is the default. Only `Original` is the disc's behaviour. That one is proved byte for byte at the pad
  and field for field in flight.
- **The switch attitude and the 0.2 margin are the port's feel, not measured.** The original never asks
  this question, so there is nothing of its to match. Nobody has flown it in the shell by hand. The
  margin is a guess at keeping knife-edge flight from swapping the stick every frame.
- **A loop flies differently under `New`.** Past the vertical the plane's up goes below the horizon, so
  the stick turns round part way through a loop. Holding the stick back over the top therefore stops
  pulling through, and the player has to push over the top instead. That is what "the same on the
  screen" means for a camera that keeps the world's up. It is the design, but it was never flown or
  judged by a person.
- **The aileron is left alone because of the reasoning above, not a measurement on screen.** No
  screenshot was compared. The flight test measures only the elevator and the rudder.
- **The plane's own bindings were not compared with the original.** That stick back lifts the nose and
  that L1 is the "rudder left" command come from I6's reading and the port's simulation. Neither was
  checked against the original live: R8's "sign of Climb" gap is still open. P50 never needs that sign,
  because it compares the port with itself the right way up.
- **The Controls screen's combined row is the port's front end.** It renders in the same way as the
  other rows, but I took no screenshot of it.
- **Online play.** Each peer remaps its own pad before sending it, so a remote seat's input already
  arrives turned round. No netplay test flies a plane upside down.
- **No emulator was used this task.** The finding about the original rests on the decompilation and
  on S34's proved port of the control law.

## Next

None. If the loop behaviour turns out to feel wrong, the switch could follow the camera's own frame
rather than the world's up, but that would be a change of feel, not of correctness.
