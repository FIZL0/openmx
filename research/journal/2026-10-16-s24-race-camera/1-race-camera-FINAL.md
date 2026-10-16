# 1 — race camera (FINAL)

Task S24. 2026-10-16.

## What I did

Ported the follow camera the disc actually creates — the derived class whose frame S23 found — beside
the base class S7 ported, and pointed the shell at it.

* `crates/game-sim/src/camera.rs` gains `Race`, a struct holding the base `Follow` and carrying the
  routines the derived class rewrites: `aim`, `limit_distance`, `look_at`, `position` and the frame
  `update`. Two small input structs come with it: `Ridden`, which is a `Target` plus the two
  fields this frame reads off the actor instead of working out (the grounded flag and the speed), and
  `Switches`, the two engine-wide bits the frame reads that are neither the rider's nor the preset's.
  Nothing in `Follow` moved or changed.
* `crates/game-sim/tests/camera.rs` gains four proofs at the end of the file — the derived aim, the
  derived look-at, the derived position and the whole derived frame — on the same harness S7 wrote,
  with a longer compared camera region (the derived object runs to `+0x1c4`) and three globals poked.
* `crates/openmx/src/main.rs` now builds a `Race`, hands it `ride.air.on_ground` and
  `ride.chassis.speed`, and asks it for the frame. It also sets the camera's snap flag once before
  the first frame — see below.
* `research/oracle.txt` gains an S24 block: `race_camera_aim`, the mode word the aim's split-screen
  trim tests, the look-ahead bit, the frame step by step, and both of the routines S23 had named but
  not read. It also corrects one claim of my own first draft: the derived aim, look-at and position
  are **not** vtable slots — `./kit xref` shows the frame is each one's only caller.
* `PLAN.md`'s code map says which of the two classes is the one that runs.

## What the original does

The frame is `./kit fn 0x0021e880`; its three routines are `./kit fn 0x0021efc0` (aim),
`./kit fn 0x0021f320` (look-at) and `./kit fn 0x0021f600` (position). The constructor is
`./kit fn 0x0021e1d0` and the class's vtable is the data at `0x00452c20`, whose `+0x38` holds the
frame — the same slot the base class's secondary vtable holds `camera_update` in.

It is a near-copy of the base class's frame, not a subclass that adds a step. Measured against the
frame S7 proved:

* **The distance floor** is the viewport's near plane times four and a half, where the base reads the
  plane raw. On the disc's own viewport (2.4 feet) that is a floor of 10.8.
* **The speed is read off the actor**, not taken as the length of the velocity, and the **heading**
  swings over to the velocity once the rider is grounded and faster than 20. There is a second,
  lower switch for the **direction basis** the camera is actually pointed along: grounded, faster
  than 5, and the speed far enough from the rider's bare vertical speed that flattening the velocity
  leaves something to normalise.
* **The lean swing** that the look-at rides does not carry the preset's `CameraSwing`. The aim still
  multiplies the yaw by it, so the preset is not ignored — only this share of it.
* **Reverse** is one sign test: above speed 4, and not on a look-back preset, if the velocity points
  against the heading the velocity becomes the constant quad `(0,0,0,1)` and the speed becomes zero.
  No arc cosine, no table, and no ramps. The three reverse ramp fields are still *read* (the distance
  adds `reverse_following`, the aim does not) and nothing in this frame ever writes them.
* **The aim** has no reverse term in its pitch, and two trims the base has neither of after the
  ±1.4835324 clamp. The first is the low-speed one: below the preset's `LowSpeedAngleXVelCutoff` the
  pitch is scaled towards `LowSpeedAngleXScaleFactor` as the speed goes to zero and then that factor
  times the rider's bare forward tilt is taken off, so a bike stopped on a slope does not stare into
  the hill. **That clause is the first reader found for either field** — S7 recorded both as having
  no known reader. The second trim takes a third of the preset's pitch off in a window less than
  0.55 as tall as it is wide, vetoed by a mode word and by a flag on an application sub-object.
* **The look-at** leads by the plain speed over the camera's speed reference, with no cap at one and
  no projection onto the heading; only a fall faster than 9 corrects it, and that correction is
  divided by 1.8; the result is floored at zero after the preset's `LookAheadDistance`. It swings the
  **direction basis**, not the heading, and does not normalise it afterwards — the base's extra
  `rsqrt` over an already normalised rotation was only rounding. A look-back preset takes a zero lead
  and then runs the whole routine, where the base returns early and skips the ground probe with it.
  The point it leads from is the target point pushed one step along the velocity (a step of at most
  a fifth of a second).
* **Standing still** — below speed 0.0001, or whenever bit 3 of the byte at `+0x325` of the
  application object is set — the look-at is the plain target point lifted by the height bias, with
  no lead and no ground probe at all, and the *position* leads instead, by ten steps of velocity.
* **There is no push-out.** The frame does not test the collision object and has no probe block. The
  derived class does build a collision object, and overrides the vtable slot the base class stubs
  with a real probe-and-collide routine (`./kit fn 0x0021e4d0`) — the frame never calls it.

What it does *not* override: the field of view, the eight filters, the preset handling and the
nudges. Those are the base class's, reached here through the base object.

## How it was proved

Function oracle, on S7's harness in `crates/game-sim/tests/camera.rs`, 4000 generated cases each:

* `the_race_aim_is_the_games_own` — 2374 of 4000 cases took the low-speed pitch scale, 1135 the
  split-screen trim, 864 were short windows one of the two vetoes stopped.
* `the_race_look_at_point_is_the_games_own` — 1007 were the look-back cam, 2510 fell fast enough to
  shorten the lead, 955 of those far enough to meet the clamp at zero. The ground probe is hooked,
  as S23's proof of the base look-at hooks it and for the same reason (the real slot wants a world
  object and a terrain tree): the port runs first over a `Dirt` that answers a chosen lift, and the
  hook hands the original that lift, writes it where the real routine writes it, and notes the point
  it was asked about, which is compared bit for bit with the point the port asked about.
* `the_race_position_is_the_games_own` — 2026 of 4000 stood still and took the position's own lead.
* `one_race_frame_is_the_games_own` — the whole frame: 889 took the heading off the velocity, 2170
  tested the velocity's sign and 621 of those threw it away, 1550 stood still, 2215 had the near
  plane beat the preset's minimum distance.

Each case writes the generated camera, filters and scratch into the interpreter's RAM, runs the
original, and compares the three regions word for word against the same state with the port's answer
written over it — the camera region extended to `+0x1c4` so that the derived object's whole tail,
including the lift at `+0x18c`, is compared. Every call is checked for writes outside those regions.
The frame proof also compares the `View` the port returns against the three values the original hands
its viewport, through hooks on the viewport's two setters.

`./kit check`: 446 tests pass. Beside that, the shell's own first frame
(`cargo run -p openmx -- --seconds 2 --size 320x240 --shot …`) draws the bike from behind on the
start line, which is what it drew before.

## Not verified / not 1:1

* **What the look-ahead bit means.** The frame tests bit 3 of the byte at `+0x325` of the
  application object. It is cleared by the mode teardown (`./kit fn 0x00202da0`) and written from a
  parameter by `./kit fn 0x00207770`, which has 46 callers; I stopped looking rather than read all
  of them. It is an input (`Switches::no_look_ahead`) and the shell passes `false`, so the shell
  never takes the branch it gates — the branch itself is proved, by the frame's 1550 standing-still
  cases, which reach it both ways.
* **The split-screen trim is unreachable in the shell.** `Switches::short_viewport` is computed from
  the window's own two extents, which is the test the original makes of its viewport's, but the
  shell never opens a window that shape and I have not run the game split-screen to see the trim
  fire for real. Its two vetoes — a mode word against 3 and a flag on the object at `+0x3cc` of the
  application object — are passed in as booleans and their meanings are not known.
* **The grounded flag and the speed are read as S23 read them**: the actor's `+0xb0` and `+0xb4`.
  Nothing here proves those two fields are what the ride writes `air.on_ground` and `chassis.speed`
  into — the oracle proofs hand the original whatever the port hands itself, so they would pass
  whatever the two fields turn out to mean. The shell's choice of the port's two equivalents is a
  reading of S23's note, not a proof.
* **The ground probe is hooked, not run**, in both of the proofs that reach it. Its arithmetic was
  read off the disassembly by S23 and ported then as `Follow::probe_ground`; what is proved here is
  the point it is asked about, the camera field it writes and where its answer lands. The heights
  the real routine would get out of the world's own ground query are not proved against anything.
* **The derived `+0x108` override (`./kit fn 0x0021e4d0`) is not ported.** The frame never calls it,
  so nothing on screen depends on it; it rebuilds the camera's probe bundle and collides it, and
  whatever calls it is somewhere else in the engine.
* **The reverse ramps are dead weight in this frame.** `reverse_following` is read by the distance
  and the other two by nothing, and no path in this frame writes any of the three. A camera that
  only ever runs this frame keeps them at zero. They are left on `Follow` because the base class's
  frame, which is dead code on the disc, still writes them.
* **The two cameras' float association differs in one place and that is as written**: the base
  computes the look-at's vertical term as `(1 - ratio) * vel.y * 0.25` and this one as
  `(1 - ratio) * (vel.y * 0.25)`. Both are the bits their own routine produces; neither is a
  rounding of the other.
* **The shell snaps the camera once before the first frame**, which the original does not do at that
  point. Without it the eight lags start at zero and the camera crawls out of the world origin over
  about a second — visible now only because the base class's push-out used to yank it back, and this
  frame has no push-out. The disc gets its snap from the race setup applying a preset over a
  previous one; the port's constructor applies its first preset to itself, and `set_preset`'s own
  test cannot fire on that. Which call on the disc sets the flag for a race start was not read.
* **`Follow` is left exactly as S7 and S23 left it**, including its frame, so the port still carries
  a frame the disc never runs. Removing it is not this task's call: its proofs are what the derived
  class's reading was measured against.

## Next

Nothing outstanding for the camera's frame. Two things a later task could pick up: what sets the
look-ahead bit (it is the one input here whose meaning is unknown), and whether anything on the disc
calls the derived class's `+0x108` override.
