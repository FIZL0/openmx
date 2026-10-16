# 1 — camera probes (FINAL)

Task S23. 2026-10-16.

## What I did

Gave the follow camera something to stand on, and gave the window its real shape.

* **Answered the brief's question.** The argument S7 called `nearest` and the shell was handing zero
  is **the viewport's near clip plane**, not a collision distance. Nothing probes for it. See below.
* **Found and ported the probe that does ask the scene for a height.** It is vtable slot `+0x10c`,
  which S7 found a `return 0.0f` *in the class it read* — and overridden in the class the game
  actually creates for a race. `Follow::probe_ground` is that override, asked about
  `Follow::ground_probe_point`, and its answer goes into the look-at's height where the port used to
  add a literal zero.
* **Ported the camera's other question about the world**: the base class's post-filter push-out,
  `Follow::push_out` over the new `camera::Obstacles`, which is the only part of the camera that asks
  where anything solid is. (It may be dead on retail — see *Not verified*.)
* **Answered both from the scene the shell already has.** `camera::Surface` and `camera::Obstacles`
  are implemented for `world::Ground`, `world::Props` and `world::Place` in `game-sim/src/world.rs`:
  the ground question is the terrain query the wheels use, and the obstacle question is the terrain
  marched along the segment against S4's sweep over the scene's 353 collision shapes, whichever hit
  comes first. The shell passes `&mut world.place` and gets both.
* **Made the field-of-view conversion read the window.** `game_render::world::vertical_fov(horizontal,
  aspect)` is now one function, used by both `Camera::chase` and the shell, and the shell hands it the
  frame's own width over height instead of 4:3. The authored angle stays the horizontal one, which is
  what the original's projection does with it, so a widescreen window sees *more* top and bottom than
  4:3 did rather than a stretched 4:3 picture.
* **Proved the probe.** A thirteenth proof in `tests/camera.rs`,
  `the_ground_probes_point_is_the_games_own`, which hooks the vtable slot so the original can be made
  to answer a chosen lift. The other twelve are untouched and still green.

## What the original does

Recipes: `./kit fn <addr>`, `./kit xref <addr>`.

**The argument.** The base frame (`0x001fa4c0`) floors the follow distance with
`max(preset MinFollowingDistance, viewport+0x19c)`. `viewport+0x19c` is the **near clip plane**: the
viewport constructor `0x00112310` puts 1.0 there and 2.0 at `+0x1a0`; `0x00112a80(value, viewport)`
writes `+0x19c` alone and pokes the viewport's `+0xc0`; `0x001131d0(near, far, viewport)` writes both
and builds `1 / (1 - near/far)` into the projection, which is a depth range and nothing else. Its
three retail call sites are all in `0x0020aaa0`, each guarded by a `... + 4 == 5` test on a global
chain, and each passes `0x4019999a` = **2.4**. So S7's reading ("whatever the camera's collision probe
says is the nearest it may sit") is wrong: no probe answers that argument.

**Which class the game runs.** The object created for a race is *not* the class S7 ported. Its
constructor is `0x0021e1d0` (called from `0x001ff270`) and its vtable is `0x00452c20`, which overrides
the base `0x00452060` at `+0x008`, `+0x038`, `+0x07c`, `+0x094`, `+0x0dc`, **`+0x108`**, **`+0x10c`**
and `+0x12c`. Its frame is `0x0021e880`, and the line that answers the brief is

    distance = viewport+0x19c * 4.5     (0x0021e880, then the same min/max against the preset)

so on the disc the floor is `max(MinFollowingDistance, 10.8)`.

**The ground probe** — `0x0021e7d0`, vtable slot `+0x10c`, the override of the base's `return 0.0f`
(`0x001fb0a0`):

1. copy the point it was handed and call the world's ground query (`0x001dc960`, the one the wheels
   use, with a null normal, no flags and no record) on the copy — it moves the point onto the surface
   under it;
2. `if (point.y < snapped.y) lift = (snapped.y - point.y) * 0.6`
3. `else lift = lift * 0.02`, where `lift` is the camera's own `+0x18c`, i.e. last frame's answer.

So it is a one-frame-lag climb out of the dirt (six tenths of the error each frame) and a fast decay
out of it (a fiftieth) when the ground is below the point again.

**Where it is asked.** Inside the look-at, base `0x001fb600` and derived `0x0021f320`, which build the
probe's point identically:

    probe point = swung_heading * ((|pitch| / 1.5707964) * 1.2 + 0.3) + target point

on the *same* normalised swung heading the look-ahead point rides. The two classes then differ in one
float: the base writes `y = y_term + lift + height_bias` and the derived writes
`y = y_term + height_bias + lift`. The port is the base's order, because the base is what the proofs
run.

**The obstacle probe.** The base frame's last act, after the lags: build the probe segment from
`target_point + rider_up * 2.0` to the filtered position, hand both points to the camera's collision
object (`0x001e8850` sets a bundle's `2 * count` points at `+0x128` and a float each at `+0x124`, and
zeroes them when the third argument is null), refresh it (`0x00119be0`) and collide it against the
world (`0x0011a150`). On a hit the frame overwrites the position it is about to hand the viewport with

    hit - normalize(end - start) * 2.0

normalised through the **EE FPU** (`mula.s/madda.s/madd.s` for the length squared, then `rsqrt.s`),
not VU0's approximate reciprocal square root, with a zero-length segment leaving the direction at the
constant quad `(0, 0, 0, 1)` instead of dividing by zero. The lags are *not* told, so the next frame
filters from where the camera would have been in the open.

**The field of view.** The projection setter takes the *horizontal* angle, makes the half width at the
near plane as `near * tan(fov / 2)`, and divides that by the viewport's aspect for the half height. So
`cpresets.ini`'s `FOV` is horizontal on every shape of viewport and the vertical angle follows the
viewport — which is also what makes the game's own split screens work. At 4:3, `Preset1`'s 85 across
is 68.997 down, which is what the human's save state recorded (V2) and what the port already drew.

## How it was proved

`./kit check` — **441 tests**, green, including `tests/camera.rs`'s thirteen.

The new one, `the_ground_probes_point_is_the_games_own`, over **4000 generated cases**: the harness
hooks the vtable's `+0x10c` so the hook writes down the point the original asks about and hands back a
lift; the port runs first over a fake ground (`Dirt`) that answers a chosen lift, and the hook then
feeds the original *that* lift. Both halves are compared — the point the probe was asked about, lane
by lane, and the look-at the original then writes. More than a hundred cases take the lifting arm (the
test asserts it, so it cannot pass vacuously: with a zero lift it would be
`the_look_at_point_is_the_games_own` again).

The twelve S7 proofs still compare the whole camera object and the whole view, unchanged, because over
`camera::Nothing` the ported probe answers `0.0 * 0.02` — bit for bit the stub's `return 0.0f`.

## Not verified / not 1:1

* **The lift's own arithmetic is read, not run.** The `* 0.6` / `* 0.02` arms of `0x0021e7d0` come off
  the decompilation. Running the original's probe needs a live world object and a terrain tree at
  `+0x184`, which the camera harness has no way to build. What *is* proved against the original is the
  probe's point, that the probe is asked at all, and where its answer lands in the look-at.
* **The heights the probe reads are not the original's.** `world::Ground::at` is P2/D6's bilinear
  blend of a terrain cell; how the game's terrain *collision* class picks a point inside a cell is
  still unread. So the probe asks the right question and gets this port's answer to it.
* **The class the game creates is still not ported** — only its ground probe is. Reading
  `0x0021e880` and `0x0021f320` turned up four more differences from the base class S7 ported, none of
  them touched here: the look-ahead uses the speed it is handed rather than the velocity along the
  heading and is not capped at one; there is one fall correction (`vel.y < -9.0`, divided by 1.8)
  instead of two (`±5.0`); the look-ahead is clamped at zero; and a look-back preset is handled by a
  zero look-ahead instead of an early return down the reversed heading. The height bias and the lift
  are also added the other way round. S24's plan line carries one more, from an earlier read of the
  same frame that this session did not go back and re-check: its aim takes the rider's grounded flag
  and speed off the actor rather than being handed them. That class is a task of its own — S24 — and
  this entry is the brief for it.
* **The push-out may be dead on retail.** `cam+0xe8`, the collision object the block is gated on, is
  zeroed by the base constructor (`0x001f9910`) and nothing was found that fills it; no writer was
  found for the hit flag at `+0x170` or the hit point at `+0xb0` (the listener that would copy them in
  is reached through the collision object's own callback list); and the frame the game actually runs,
  `0x0021e880`, has no such block at all. Its arithmetic is read off the disassembly and not run. It
  is ported and wired anyway, because the shell has to keep the view out of the props and this is the
  only code in the class that does — if a later task proves the block dead, the shell should stop
  calling it rather than the module lose it.
* **The obstacle answer is not the original's collision world.** The terrain half is a stepped march
  with a bisection over `Ground::at` (`world::Ground::under`, this port's own); the prop half is S4's
  sweep over the scene's 353 shapes. The original sweeps a one-probe bundle of the same collision
  class through its own broad phase. Back faces are culled in both, so a probe that starts underground
  comes out unobstructed either way.
* **The shell passes its own near clip, not the disc's.** `openmx` hands the camera its rasteriser's
  near plane (0.05 ft), so the distance floor is the preset's `MinFollowingDistance` alone. The disc's
  race viewport would make it 2.4 × 4.5 = 10.8 ft. Two things stopped me using that: what the `== 5`
  test in front of all three 2.4 call sites selects is unknown, and whether 10.8 ever beats a preset's
  `MinFollowingDistance` was not checked — the presets are inside `MX.PSR` and I did not read them out.
  If it does beat them, the retail camera cannot be zoomed in as close as this shell's can.
* **The field of view at a non-4:3 window is not compared against anything.** The disc renders 4:3 and
  nothing else, so there is no original frame to hold a widescreen window up against; what is checked
  is that the conversion is unchanged at 4:3. The choice itself — keep the authored horizontal angle
  and let the vertical follow the window — is the original projection's own behaviour, and the
  alternative (keep the vertical and widen the horizontal) would crop the top and bottom off the
  disc's picture.
* **Nothing was looked at by eye.** No capture, no screenshot, no emulator run: this task's whole
  surface is either proved against the original function or listed above. The one thing a human could
  usefully do is ride a dip in both and say whether the lift feels like the game's.

## Next

Nothing is waiting on this file: the task is finished and the entry can be renamed `-FINAL`. The
follow-up it leaves behind is the third bullet above — port the camera class the game actually creates
(ctor `0x0021e1d0`, frame `0x0021e880`, look-at `0x0021f320`, position `0x0021f600`, vtable
`0x00452c20`), whose five remaining differences from S7's base class are listed there.
