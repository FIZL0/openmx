# 1 — trick camera orbit (FINAL)

Task P319. 2026-11-10.

## What I did

- **Fit by projection, not by a sphere** (garage_view.rs `fit_distance`, shared by both pages). Both stages
  used to stand back far enough for a sphere round the model's box diagonal, which leaves the frame half
  empty. Now the fit takes the model's own points (`points_of`: the bike's, every part's at its node and the
  skinned rider's, a few thousand corners in all). For each direction the camera will look from, it finds
  the nearest distance at which every point lies inside the frame's height and width, less a 5 % margin
  (MARGIN). It keeps the largest of those over the directions it is given. The fit is exact for a pinhole
  camera: d = max over points of max(|q·r|/tan_x, |q·u|/tan_y) + q·e.
- **Garage** (P213): the slow turn is unchanged (TURN 0.4 rad/s). The fit runs over 48 headings of that turn
  at the stage's own aspect and is cached per aspect. The camera aims at the middle of the points' box, not
  at the model's origin. The stage rect (GARAGE_STAGE, menu_screens.rs) is unchanged. It is bounded by the
  plate on its left (x 45..58.6, y 4..15.8), the side text below it (from y 25.2) and the screen's edge, so
  the bike can only grow within it, and now it fills the stage's width at every heading. At 1920x1080 it is
  about 1.27× as wide as before.
  - **Phone (2340x1080):** about 1.15× as wide. It is held to the 16:9 frame inside the phone's wider
    screen.
  - **Tried and dropped:** running the stage on into the phone's side room, out to the screen's edge. At
    that width the fit became height-bound, because the nearer wheel looms as the bike turns towards the
    camera. The bike stayed the same size and only moved right, so the stage rect stays as it was.
- **Tricks** (P259):
  - Each trick's whole loop is swept once, when it is first lit (`sweep`: 13 poses over the loop, with the
    skinned bike, its parts and the rider at each). The camera aims at the middle of all they reach.
  - At each pitch, the distance fits every point from all 32 headings round (`Swept::back`), cached per
    stage aspect and per 0.05 rad of pitch. So nothing leaves the frame at any yaw, through flips and 360s
    included.
  - The fit does not cover every pitch at once. Changing the pitch refits, and the distance eases there
    (SETTLE 8/s) rather than jumping. Fitting the whole pitch range would have kept the rider small at
    the resting view.
  - At 1560x720 the rider is about 1.6× as big as before.
- **The orbit** (`Orbit` in trick_view.rs, one per shown seat):
  - Left alone, the camera turns round at the Garage's TURN pace.
  - The right stick takes it over: yaw at 2.4 rad/s, with a 0.2 dead zone, and pitch within −0.35..1.1 rad.
    The camera stays where the player leaves it.
  - HOLD (5 s) after the last stick or drag input, the orbit starts again from where it is. Its pace eases
    in with a smoothstep over 1.5 s (EASE), so there is no jump in yaw or in yaw rate.
  - The directions follow game convention. Stick right takes the camera round to its right; stick up tips
    the camera up (it looks from lower).
  - A drag (mouse on the desktop and in the browser, a finger on the phones) pulls the stage under the
    pointer: a stage-width drag is half a turn.
- **Pointer plumbing.** The desktop window already reported `mouse()`. The browser (index.html pointer
  events → worker.js `mouse` → web_host.rs / web_gpu.rs), Android and iOS (touch_pad.rs: a finger that
  lands off the pad's buttons and stick is a click *and* the start of a drag) returned nothing before.
  Now they report Down/Move/Up.
- **The right stick had no menu use.** The menus read only the left stick (axes 2, 3) and the d-pad, so
  nothing had to move to another input. The pause hands the trick page each seat's report
  (`trick_sticks`, main.rs), the tape's in a `--play` run.
- A line `tricks camera: 1P yaw … pitch … orbiting|held|easing back, t s in` is printed once a second and on
  every change of state.

## What the original does

Nothing to match. Both stages are the port's own (P213, P259). The original's garage has no rider/bike
stage like this and no trick viewer.

## How it was proved

- Unit tests in trick_view.rs:
  - `the_orbit_turns_holds_for_the_stick_and_eases_back`: idle pace = TURN; the stick turns and tips it;
    it is held to the bit for HOLD s; afterwards there is no jump, a rate that only grows and ends at TURN;
    the pitch limits hold.
  - `the_right_stick_leans_off_axes_0_and_1`.
  - `a_fit_keeps_every_point_in_the_frame`: 4 aspects × 4 pitches × 32 headings; every point lands inside
    the frame, and one lands within 10 % of its edge.
- Menu shots, before and after, by `--menu --screen garage|tricks --shot` on a private headless weston
  (llvmpipe), at desktop and phone sizes. Garage at 1920x1080 and 2340x1080; Tricks at 1280x720 and
  1560x720 with `--track free06`, because the default track takes too long to load on llvmpipe. Not
  committed (game pictures).
- A hand-written `--play` tape on the Tricks page: idle 2 s, right stick full right 1 s, then released
  10 s. The log (`tricks camera:` lines, 1560x720), trimmed:

      yaw 70.7  pitch 14.8 orbiting,     0.0 s in
      yaw 93.3  pitch 14.8 orbiting,     1.2 s in
      yaw 115.7 pitch 14.8 orbiting,     2.1 s in
      yaw 148.6 pitch 14.8 held,         2.4 s in    <- stick right
      yaw 247.3 pitch 14.8 held,         3.1 s in    <- stick let go
      yaw 247.3 pitch 14.8 held,   4.0 … 8.0 s in    (unchanged every second)
      yaw 247.3 pitch 14.8 easing back,  8.1 s in    <- 5 s after the last input
      yaw 254.5 pitch 14.8 easing back,  9.1 s in
      yaw 267.0 pitch 14.8 orbiting,     9.7 s in
      yaw 297.8 pitch 14.8 orbiting,    11.0 s in
      yaw 345.1 pitch 14.8 orbiting,    13.1 s in

  The idle pace is about 23°/s, TURN's 0.4 rad/s. The tape's 30 stick lines turned the camera 99°, which is
  0.72 s at 2.4 rad/s, so the stick read as held for less than its 1 s of tape lines. The pause draws
  frames at its own pace while the tape feeds one report per tick, and the orbit runs on wall time, not
  on tape ticks.

## Not verified / not 1:1

- **No device or browser tried the drag.** The phone (Android, iOS) and browser paths were only compiled,
  for aarch64-linux-android with the NDK, aarch64-apple-ios with tools/ios-build.sh, and wasm32 with
  /tmp/openmx-cross's rustc. Nobody dragged on a phone, in a browser, or with a desktop mouse. The tape
  drove only the right stick. On a browser phone, web/touch.js's pad covers part of the canvas whenever
  it is up; a drag that starts on its stick zone goes to the pad, not to the camera.
- **No real pad turned the camera.** Only the tape's report, and the unit tests, drove the stick.
- **Speeds and limits were picked by eye, not by any rule or original:** stick 2.4 rad/s, dead zone 0.2,
  pitch −0.35..1.1 rad, a stage-width drag = half a turn, ease 1.5 s, distance settle 8/s, 5 % margin.
- **The stick's direction is a choice.** Right takes the camera round to its right and up tips it up, as
  most games' orbit cameras do. The drag goes the other way: it pulls the stage.
- **The fit is per pitch.** At a fixed pitch nothing leaves the frame at any yaw, through the whole loop.
  Changing the pitch eases the distance to the new fit at 8/s, so for a moment during a fast pitch
  change the motion can touch the frame's edge.
- **The orbit runs on wall time.** A `--play` tape does not replay the camera to the bit: how far the
  stick turns it depends on the frame pace. It is a picture, not sim state, so nothing that is hashed
  reads it.
- **Few poses are sampled.** The Tricks fit samples 13 poses over the loop and about 500 points per pose
  (FIT points 3000 on the Garage). A limb between samples could poke past the 5 % margin by a little.
  Nothing did in the shots.
- **Only part of the matrix was shot.** Tricks shots: free06 only, the first trick lit, one seat. The
  Garage shots used the default pick; no Fliers tab or split-screen views were shot.

## Next

Nothing for this task. For a human: try a drag on a phone and in a browser, and the right stick on a
real pad, on the pause's Tricks page.
