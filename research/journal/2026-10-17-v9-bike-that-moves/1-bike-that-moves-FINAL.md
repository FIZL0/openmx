# 1 — bike that moves (FINAL)

Task V9. 2026-10-17.

## What I did

Nothing on the bike moved. The shell placed each part of a `.sub` at its chassis node's *bind*
transform — `chassis.model.world(i)`, once per frame, off the static model — so the forks never
compressed, the wheels never turned, the front end never steered and the swingarm never swung. S25
had ported the vehicle's animation machine, and nothing consumed its pose.

Both halves are in one new module, `crates/game-render/src/vehicle.rs`, because the original keeps
them in one function each and runs them in that order: the blend first, the drives second.

**Half one — the chassis is posed from its driver.** `Chassis` holds a `Set` (the bike's own `.mcf`
animation set, with its `SLTFile` skeleton), a `vehicle_anim::Library`, a `vehicle_anim::Driver`, a
`motion` `Player`, and the `Vec<M4>` the last frame left on every node. `Chassis::frame` runs
`Driver::frame`, then `Blend::run` on the player, then the new `Set::apply_vehicle` turns the
driver's `Blend` into a `Pose` the same way V8's `Rig::driven` turns `motion::Blend` into the
rider's. Four of the five blend modes draw; see below for the fifth.

Three small additive pieces made that possible:

- `scene.rs` now reads `[Objects] MCF` off the `.sub` (`bike.mcf` on every `.sub` on this disc). It
  was the one genuinely missing field — the chassis model comes from that set's own `SLTFile`, not
  from the `.sub`.
- `skin.rs` grew `Skeleton::compose(&[M4])`, the same walk down the tree `world` does but from local
  transforms the caller built itself. The drives need it and `resolve` is private.
- `skin.rs` grew `Clip::pose_paired_faded`, a two-phase sampler: `lay`'s `mixed` became
  `Option<(&Clip, f32, f32)>` so the second clip can be sampled at its own time. That is what
  `Blend::Pair` (mode 5) means and without it mode 5 would have drawn nothing.

**Half two — the suspension, wheels, steering and swingarm come from the simulation.** `Drive::of`
reads seven numbers off a stepped `Ride`; `Chassis::drive` writes five nodes, in the original's
order, over the locals the pose just produced, then re-composes:

| node | what is written | from |
| --- | --- | --- |
| `Handlebars` | rotation about the fork axis | `front.w.steer_angle`, only while on the ground |
| `Shocks` | translation, `rest + fork * (inv_sin * c)` + a fixed nudge, put back into the bars' frame | `front.w.compression` |
| `TireFront` | rotation about x | its own integrated angle |
| `SwingArm` | the whole local: a look-at from the pivot to the rear mount, lowered by the compression | `rear.w.mount`, `rear.w.compression` |
| `TireRear` | rotation about x | its own integrated angle |

The two wheel angles are integrated here (`Roll`), with the original's own wrap, because the
simulation keeps a spin *rate* and the ported wheel never kept the angle.

`openmx/src/main.rs` loads a `Chassis` beside the models, calls `chassis.frame(...)` each frame
while riding, skins the chassis model from it, and draws each `.sub` part at the **live** node
(`node_now`), falling back to the bind transform when there is no chassis.

## What the original does

A frame of the chassis is three functions.

`./kit fn 0x00248a70` is the vehicle's animate entry: the `want` writer (the idle/taunt random
timer and the state picks) and then a call into `./kit fn 0x002492b0`, which is S25's ported
`Driver::frame`. The `want` writer is still not ported — see below.

`./kit fn 0x00258d50` is the snapshot: it copies the simulation's own numbers out of the two wheels
(`+0x7b0` front, `+0x7b4` rear) into a flat block on the vehicle — the two spin angles to `+0x878`
and `+0x884` from each wheel's `+0x1d0`, the compressions to `+0x86c`/`+0x87c`, the steer angle to
`+0x874` from the front wheel's `+0x8ac`, and the on-ground flag to `+0x93c` (bit 0 of `+0x8a1`,
which is the actor's `+0x574`). That block is the seam `Drive` stands for: the renderer reads the
snapshot, never the wheels.

`./kit fn 0x002593e0` is the drive. In order:

- the fork target is built in the `Bike` frame as `+0x9b0` + `+0x9d0 * (+0x9e0 * +0x86c)` +
  `_DAT_00420be0`, then `FUN_001b3060` takes node `+0x964`'s inverse and `FUN_001b2660` writes the
  result as node `+0x970`'s translation. `+0x9b0` is the rest position, `+0x9d0` the normalised fork
  axis, `+0x9e0` the one-over-sine, `_DAT_00420be0` the quad `(-0.03, 0.0, 0.02, 1.0)`.
- `if (+0x93c)` — on the ground — `FUN_001b2e50(+0x874, +0x968, +0x9d0)` turns the bars about the
  fork axis by the steer angle. In the air the bars are left wherever the pose put them.
- `FUN_001b2e50(+0x878, +0x974, 0x00426da0)` and the same for `+0x884`/`+0x978` turn the two wheels.
  `0x00426da0` is the quad `(1, 0, 0, 1)`: the x axis.
- `FUN_001b1d70(+0x96c, dir, 0x00426db0, 1, 1, 1)` aims the swingarm, where `dir` is the normalised
  `+0x9c0 - (mount.x, mount.y + +0x87c, mount.z)` built inline with `vsub`/`vrsqrt`/`vmulq`, and
  `0x00426db0` is the quad `(0, 1, 0, 1)`: y up.

The load-time geometry is `./kit fn 0x00253760`, the same function S22 found the wheel mounts in: it
looks the nodes up by name in the bike's own frame, and `Shocks` and `Handlebars` become the
normalised steering axis.

## How it was proved

`crates/game-render/tests/vehicle.rs`, four tests off the real disc (they print `SKIPPED: …` and
pass when there is no disc, the idiom `tests/body.rs` uses). A `250sx` chassis, `appstart.res`'s
`stunts.ini`, a `cr125_faisst` `.phy`, dropped from 30 units and ridden on the throttle:

- **a minute of riding poses the chassis every frame** — 3600 frames, every transform on every node
  finite, the machine in states `{0, 30, 35}`, blend modes
  free/one/two/_/four/pair `= [0, 79, 3521, 0, 0, 0]`.
- **the fork node travels with the ported compression** — the fork axis comes out
  `(0, 0.8808, -0.4735)` with `inv_sin` 1.1354; the node moves 1.1353 along it over the minute and
  every frame's position is within 1.79e-4 of `rest + fork * (inv_sin * compression)` recomputed
  from the compression the simulation had that frame. Steady-state front compression is 0.46
  against a 1.2 tire radius — about 38% sag, and ~0.52 units of visible fork travel on a ~5-unit
  bike.
- **the wheel node's angle integrates its own spin** — 36.9 and 48.0 turns over the minute, ending
  at -1.093 and -0.064 rad, each frame's angle matching the rate the simulation reported.
- **a stunt's first and last frames differ in the bike's pose** — `Taunt`, 1.670 s / 79 frames, the
  machine in its gesture slot and nothing else for every frame of the trick, blend mode 1 ("play the
  new clip alone", correct because the previous state was 35 and not another gesture); the furthest
  chassis node moved 3.407 between the trick's first frame and its last.

The mounts the test hands the simulation are the **chassis model's own `TireFront`/`TireRear` node
positions** (`Chassis::node_rest`, added for this), not the `.phy`'s `BodyTireForcePosition` — the
same thing `main.rs`'s `node_at` does, and the seam S22 wrote up. With the `.phy` positions instead
the mounts are `(0, -0.25, ±0.75)` in a different frame and a different scale, the front compression
never leaves zero and the rear sits bottomed out; with the node positions they are
`(0, 1.233, 2.5225)` and `(0, 1.195, -2.58973)`, which is what the original has to the bit.

Not proved against a recording or a capture: rendering only has to look right (AGENT.md), and the
numbers the drives consume are already bit-exact through the simulation's own tests.

## Not verified / not 1:1

- **The `want` writer is still not ported** (`./kit fn 0x00248a70`, the half S25 left out). What the
  state machine is asked for each frame is this port's own reading: the trick's gesture slot while a
  trick is in progress, `va::AIR` off the ground, `va::TURN_RIDING` otherwise. The original's idle
  and taunt timers, its random picks and its wreck states are all absent, so the bike never idles
  or taunts of its own accord.
- **`inv_sin` is a closed form**, `sqrt(y² + z²) / y` off the fork axis. The original reaches the
  same number the long way round, through its arc-cosine table and then its sine table, so the last
  bits will differ.
- **Every number in `params()` is this port's own**: five rates of 3.0 (what the rider's constructor
  uses), `cross_cap` 0.25, `start_phase` 0, and three zero bands. None of them is read out of the
  vehicle's constructor and none is off the disc; they are the values that make the blends look
  continuous. The fade and band behaviour will not match the original's until they are read.
- **The `bike_` prefix retry on the pose clips is a guess at a seam.** `Library::load` asks for
  `air_Mid`; this disc's `.mcf` calls it `bike_air_Mid`, so `Chassis::load` retries each absent pose
  clip with the prefix. Whether the original prefixes at that point or carries two names was not
  re-read in the decompilation.
- **`Blend::Four` (mode 4) draws nothing**, the same treatment `motion::Blend::Three` already gets.
  Mode 4 never appeared in a minute of riding or in any stunt tried, so there was nothing to look
  at; when it does appear the chassis will hold its previous pose for those frames.
- **`Blend::Pair`'s second phase is guarded for non-finiteness** and falls back to time 0. The
  original has no such guard; a clip of zero duration would give it a NaN and this port a held
  first frame.
- **The handedness of the wheel roll and of the steer** was chosen by eye. The axes are confirmed
  data (`(1,0,0)` for the wheels, `(0,1,0)` up for the swingarm look-at) and the drives are the
  original's, but whether `about()` turns the same way as `FUN_001b2e50` for a given sign was
  settled by what looked right, not by comparing a matrix.
- **`unplace` assumes the bars' transform is rigid** (it inverts by transposing the rotation). True
  of every chassis node on this disc; a scaled node would come out wrong.
- **Placement is left to the shell.** `Driver::frame` is called with `self_placed = true`, so
  `Frame::place` is always `None` and the bike's world position stays the shell's business. The
  original's placement path is unexercised.
- **Most of the bike's clips are not on this disc.** Only `bike_air_Fwd/Mid/Bck` (all three the same
  `bike_air_Mid.MOT`) and 9 of the 24 stunt clips bind — 12 clips in total. Every `bike_turn_*`,
  `bike_reverse_*`, `bike_hold_*`, `bike_get_ready_*`, `bike_backflip`,
  `bike_nohandedlander_left_right` and `bike_wreck_air_flail` is absent. That is why the grounded
  pose grid draws nothing and why a minute of riding spends 3521 frames in mode 2 on the airborne
  row: the drives are doing all the visible work on the ground, which is the half that matters and
  is also how it looks on the original.

## Next

Nothing outstanding for this task. The two obvious follow-ups, both out of V9's scope: port the
`want` writer at `./kit fn 0x00248a70` so the machine picks its own states, and read the vehicle
constructor for the real `params()` numbers.
