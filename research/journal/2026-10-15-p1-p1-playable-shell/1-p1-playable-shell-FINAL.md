# 1 — p1 playable shell (FINAL)

Task P1. 2026-10-15.

## What I did

A new binary crate, `crates/openmx`, that opens a window, reads a real pad, runs the ported frame at the
game's own 60 Hz and drops the rider into the freeride area with the renderer drawing it. No emulator, no
fixture, no menu: `cargo run -p openmx` and ride.

    cargo run --release -p openmx                                  # ride
    cargo run --release -p openmx -- --seconds 60                  # the task's own test
    cargo run --release -p openmx -- --pad /dev/input/js1 --shot context/shots/p1.png

Five files, of which four are glue and one is not:

- `main.rs` — the loop. Opens the disc named in `kit.toml`, takes `free06.res`, `free06_2.res` and
  `125sx.res` out of `MX.PSR`, reads the scene **twice** (the simulation's `track::Scene` for the spawn,
  the start direction and the terrain's layout; the renderer's `scene::Scene` for the props, the lights
  and the fog — one file, two readers, neither of them new), loads the terrain, the props, the bike and
  the parts its `.sub` bolts on, then runs `tick::Engine::step` once a frame, draws, presents and paces
  to 60 Hz.
- `window.rs` — a window with no dependencies and no `unsafe`: the X11 wire protocol spoken straight
  down `/tmp/.X11-unix/X0`. Setup handshake, `CreateWindow`, a title through `ChangeProperty`, a
  graphics context, `MapWindow`, and the frame pushed over as `PutImage` in bands that fit the server's
  maximum request. Key presses and releases come back as the 32-byte events the protocol defines. The
  socket is non-blocking and the write loop drains events while it waits, because a client that fills
  the server's buffer while the server is filling the client's deadlocks otherwise.
- `pad.rs` — the host's pad, as the game's own report. Linux's joystick interface gives 8-byte events
  with axes normalised to ±32767; those, plus the keyboard, are folded into the 32-byte DualShock 2
  report I1 reads: the active-low big-endian button word, the four stick bytes, and the twelve pressure
  bytes that twelve of the sixteen controls are actually read from. Analogue triggers become L2 and R2
  pressures. The port's `input::Manager` does the rest, so the dead zone, the strengths and the
  press/release edges the shell reads are the game's, not mine.
- `ground.rs` — the freeride area's ground. D6's terrain query answers "the four corners of the cell at
  sample (x, z)"; the shell asks it about a world point (bilinear between the corners — mine, not the
  game's) and turns the samples around the rider into two triangles a cell to draw. It is here rather
  than in the renderer because V1's own `.trn` reader reads the supercross layout and refuses
  `free06`'s: *"terrain: 842 x 842 is not a whole number of patches"*.
- `ride.rs` — **the one part of this binary that is not a port**, and it says so in its own first line.
  See the section below.

The front end is the shell's, as AGENT.md allows: Cross is the throttle and Square the brake (both
analogue, read as pressures), the left stick steers and leans, Triangle respawns, Start freezes the ride
(the original's pause is a whole front-end page, which does not exist here), L1/R1 walk the track's own
camera presets, Escape or closing the window quits. The same on a keyboard: X/Space, Z, the arrows, V,
Return.

## What the original does

Nothing new was decompiled for this task: P1 is the task that wires proved ports together, and every
behaviour it depends on was read from the game and written down somewhere else. What the loop here is
shaped by:

- The frame is the original's, through `tick::Engine` (S1): one mode request serviced before the frame,
  the wall clock sampled from EE Timer 0, `dt` as a thirty-frame moving average clamped to 30 fps, the
  pad latched once before anything simulates, the markers around the simulation, and the render in the
  app layer between the second marker and the flip — which is exactly where the draw sits here.
- The clock is sampled the way the original samples it: whole 100 ms periods and the bus-clock counts
  into the current one, handed to the engine as `Timer0 { ticks, count }` so the port's own single-
  precision `clock_ms` conversion is what produces the millisecond value.
- The pad is read once a frame, before the simulation, as one 32-byte report per port (I1, I2).
- The spawn, the start direction, the terrain's tree and its height-and-surface query are the scene and
  terrain readers proved against this very track (D6).

## How it was proved

- **It rides.** `cargo run -p openmx -- --seconds 60` (debug build, the task's own command) ran its
  sixty seconds with no panic, and `--release` holds a flat 59.7–60.3 fps at 640×448 with ~19,000
  triangles a frame. Both logs are in the session's scratch, the numbers are in the per-second line the
  shell prints.
- **The pad path, end to end, with a real device.** The kit's virtual pad for this copy
  (`/dev/input/js1`) was driven by a script while the shell ran: throttle down took the bike from 0 to
  57 mph over nine seconds, a left stick turned it (the triangle count falls as it rides away from the
  props), the brake took it back to 0 in two, and Triangle put it back on the start line — the triangle
  count returns to the start-line frame's 18,946 exactly. Every one of those passed through
  `input::Manager`, so what moved the bike was the port's own poll.
- **The picture.** The first frame is written with `--shot` and was looked at: the bike stands on the
  dirt at the scene's own start line, the chase camera is behind and above it at the preset's distance,
  the dunes are shaped by the terrain's stored normals and the scene's fog closes the distance. It is
  the same view V1's `first-draw` produces for a supercross track, with ground that `first-draw` cannot
  draw at all for this one. (`context/shots/p1-start.png`, not committed.)
- **The ride's own behaviour** is unit-tested in `ride.rs` over a synthetic flat plane and a lip: the
  throttle pulls away down the scene's heading and tops out instead of running away, the brake stops the
  bike and does not drag it backwards, a lip at speed puts it in the air and lands it slower, the edge of
  the field stops it dead rather than producing a NaN, and respawn puts it back. These prove the shell,
  not the game — there is nothing to compare them against.
- `./kit check` green.

## Not verified / not 1:1

- **The bike's ground physics is not a port.** S3 (`bike\*.phy`: throttle, brake, lean, steering,
  suspension) is unwritten, and so is the `.phy` reader that would fill in `air::Params`. `ride.rs` is
  the shell's own rider: host `f32`, invented constants in feet and seconds, deliberately outside
  `game-sim` so that no test, hash or replay can come to depend on it. Nothing in it is the game's
  behaviour — not the acceleration, not the top speed, not the turn rate, not the landing cost, not the
  lift-off threshold that keeps a bumpy field from launching the bike every few frames. When S3 lands,
  this module is deleted.
- **`air.rs` and `trick.rs` are not wired in.** Both are proved ports, and both need a bike's parameters
  to run: the hang time, the preload, the crossup and the landing test all read `air::Params`, which only
  a `.phy` reader can fill. Wiring them to invented parameters would make a proved port produce unproved
  numbers, so the shell leaves them out rather than making them look wrong.
- **S4's collision sweep is not wired in either.** The scene's 353 collision placements are read and
  counted, but nothing is swept against them: the sweep takes the motion a bike's physics produced, and
  that physics is the stand-in above. The rider therefore passes through fences, tyres and trackside
  props; only the terrain stops them.
- **The ground interpolation between samples is mine, not the game's.** D6 proved the query that answers
  a cell's four corners; the bilinear blend inside the cell is the shell's. The game interpolates a
  contact point in its wheel code, which is S3's.
- **The bike's attitude is geometric.** Pitch is the ground a wheelbase ahead against the ground a
  wheelbase behind, and roll is the terrain normal. The game's answer is the suspension's.
- **No ground texture.** The `.trn` carries a 1 MiB image and a palette, but which tile a cell takes and
  how its corners index into one is not decoded (V1 says so too), so the dirt is one flat colour with the
  file's own normals doing the shaping.
- **The ground is drawn to a radius, not to the fog.** `free06`'s fog is further away than a software
  rasteriser can fill at 60 Hz, so the dirt fades out at 44 samples (about 120 yards). The game draws its
  terrain through the tree's own level-of-detail, which is not ported.
- **The camera is V1's preset reading with no filtering.** `cpresets.ini` gives the distance, the angles
  and the field of view, and the camera is placed from them every frame. The original's camera lags and
  smooths (S7), and none of that is ported, so the view is stiffer than the game's.
- **Start freezing the ride is not the original's pause.** The original's pause is a front-end page with
  its own mode request; this shell has no pages, so Start sets the shell's own flag.
- **The pad's button order is the kit's virtual pad / xpad convention.** Which joystick button index is
  which face button was taken from that and from the convention Linux pads follow, not measured on a
  retail DualShock 2 over USB. A pad that orders its buttons differently will map differently — `--pad`
  and a different device is the test, and I had only the virtual one.
- **Select was never read.** It is the one control I1 could not measure live (the kit's pad refuses to
  send it), so the shell binds nothing to it.
- **The keyboard's keycodes assume an evdev/XKB keyboard** (the usual Linux X server or Xwayland). The
  X11 path assumes a local server that accepts an unauthenticated client of the same user, a little-endian
  32-bit-per-pixel visual at depth 24, and no window manager interaction beyond mapping a window: no
  `WM_DELETE_WINDOW` protocol, so the close button is seen as the window going away rather than as a
  request. None of that is game behaviour, but all of it is machine-dependent.
- **No sky, no rider, no shadows, no particles, no animation, no sound.** V1 draws models and terrain;
  the bike's own animation set, the rider on it and everything else are gaps inherited from the renderer,
  not new ones.
- **The scene prints its name as `Supercross01[Quadzilla]`.** That is what `free06`'s own `.scn` says in
  its `Name=` key — a leftover in the shipped data, not a sign the wrong file was read: the spawn, the
  768×768 sample field and the 9-foot spacing are `free06`'s, exactly as D6 measured them.

## Next

The shell is finished and rides; what it is waiting on is simulation, in this order:

1. **S3**, the bike's ground physics, with the `.phy` reader that comes with it. That deletes `ride.rs`
   outright: the ride becomes a `game-sim` state, `main.rs` gets shorter, and the shell keeps only the
   window, the pad, the camera and the draw.
2. **`air.rs` and `trick.rs`**, which need nothing from this crate once S3's reader can fill
   `air::Params` — a dozen lines in `World::simulate`.
3. **S4's sweep** against the scene's 353 collision placements, so the props stop the rider. It needs a
   real motion to sweep, which is again S3.
4. After those: S7's camera filtering instead of the stiff preset placement, and the renderer's own gaps
   (sky, rider, animation).
