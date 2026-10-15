# 1 — s7 camera (FINAL)

Task S7. 2026-10-15.

## What I did

Ported the follow camera into `crates/game-sim/src/camera.rs` and proved it in
`crates/game-sim/tests/camera.rs` — twelve oracle proofs, all green. The brief's three questions, and
their answers:

* **Its target** — `Follow::look_at`. The camera does not look at the rider; it looks at a point pushed
  ahead of the rider along the heading *swung by the rider's lean*, by a distance that comes from the
  speed along the heading, the follow distance, the field of view and the preset's
  `LookAheadDistance`, then lifted by the vertical speed and a constant bias.
* **Its lag** — `Follow::filter`. Eight one-pole lags: three on the look-at point, three on the
  position, one on the field of view and one spare the camera itself never reads. Their time constants
  are the preset's `LookFilters` and `PosFilters`, and a preset switch arms a *snap* that loads them
  with this frame's values instead of blending, so the camera does not slide in from where the old one
  was.
* **Its limits** — `Follow::field_of_view`, `Follow::aim` and `Follow::limit_distance`: the field of
  view against `MinFOV`/`MaxFOV`, the pitch against ±85°, the distance against
  `MinFollowingDistance`/`MaxFollowingDistance` *and* whatever the camera's collision probe says is the
  nearest it may sit. All three, on hitting a stop, rewrite the player's own offset to exactly the
  value that reaches the stop, so that leaning on the right stick against a limit does not build up an
  offset that has to be unwound before the camera moves again.

**Does it feed the simulation?** No — and that was the first thing settled, because the answer decides
how exact the port has to be. `camera_update` reads the rider's actor (body point, two basis rows,
velocity, lean) and the viewport, and writes only its own fields plus the viewport's position, look-at
and field of view. Nothing it calls reaches the game's random numbers, and nothing in it advances the
frame clock. So `AGENT.md`'s presentation rule applied and this was allowed to merely match by eye.

It is bit-exact anyway, with one documented exception (the startup sine table — see the last section).
The camera's arithmetic turned out to be plain EE FPU and VU0 macro-mode work with one call into a
table-driven rotation; porting it exactly cost no more than porting it approximately, and a bit-exact
port is a port whose proofs can be `assert_eq!` instead of a tolerance.

Three findings worth carrying forward:

* **The original computes a sideways camera swing and throws it away.** `camera_position` builds a
  yaw-driven swing over two arcs — `[0, 3pi/8]` and `(2pi - 3pi/8, 2pi]` — blends the look-at point
  and a sideways vector into it, and stores the result into stack slots that nothing reads. The answer
  the routine actually returns is `target point + direction * distance` and nothing else. The port
  leaves the dead half out; `the_positions_swing_is_dead_code` sweeps 32 yaws across 200 generated
  cameras and asserts the position never moves, which is the proof that it really is dead.
* **Two of this class's virtuals are stubs, and one of them makes a whole feature inert.** Vtable slot
  `+0x108` is `return 1` and `+0x10c` is `return 0.0f`. The second is the look-at point's ground
  probe: the original builds a probe point out of the swung heading and the pitch, calls through the
  slot, and adds the answer to the look-at height. Because the answer is always zero, the only thing
  that addition does in this class is turn a `-0.0` into a `+0.0` — which is why the port writes
  `(lift + 0.0)` rather than dropping it.
* **The reverse camera is dead data on the retail disc.** The three reverse ramps are driven by
  `ReverseAngleXOffset`, `ReverseFollowingOffset`, `ReverseLookAheadOffset` and
  `ForwardReverseSwitchSpeed`, and *not one of those four keys appears in free06's `cpresets.ini`*. A
  missing key keeps the record's zero, so on the disc the reverse branch ramps zero into zero. It is
  ported and proved all the same — 896 of the 4000 generated cases take it — because the file is data
  and a mod or another disc region could fill it in.

## What the original does

One C++ class; its `__FILE__` is `"SimpleFollowCam.cpp"` (0x0042d350), and it is reached through the
secondary-base vtable at 0x00452060. Every address, the object layout, the preset record's field
offsets and each routine's argument convention are in `research/oracle.txt` under the S7 heading; read
them with `./kit fn <addr>`. Two conventions that cost time and are worth repeating: the PS2 SDK's gcc
passes integers in `$4..$11` and floats in `$f12..$f19` with the two sequences numbered independently,
and **Ghidra lists the float parameters first**, so `camera_look_at(float, float, float, out, cam, …)`
is really three floats in `$f12..$f14` and six pointers in `$4..$9`.

Everything the camera does is driven by a *preset*: a 160-byte record read out of the track's
`cpresets.ini`. free06 ships eleven of them — `Race Cam 1`, `Race Cam 2`, `Freestyle Cam`, `Slot Cam`,
`Lookback`, `1P Stunt` and five attract cams — plus a copy of each per split-screen layout
(`Preset1h`, `Preset1v`, `Preset1w`, `Preset1x`), selected by the viewport index. `Preset1`, the camera
a race starts on, reads: `AngleY` 180, `AngleX` 65, `FollowingDistance` 13, `Min`/`Max` 5 and 100,
`FOV` 85, `MinFOV` 40, `MaxFOV` 110, `LookAheadDistance` 20, `CycleThruThisPreset` 1,
`FilterThisPreset` 1, `IsLookBackCam` 0, `CameraSwing` 1.0, `LookSwing` 1.0, `LookFilters`
0.28/0.25/0.28, `PosFilters` 0.26/0.33/0.26, `LowSpeedAngleXVelCutoff` 30, `LowSpeedAngleXScaleFactor`
0.5, `OffsetFromBodyPoint` 0,0,0. `AngleY` and `AngleX` are multiplied by 0.01745329 as they are read,
so the record holds radians.

The frame (`camera_update`, vtable `+0x98`), in order:

1. **Field of view** (`camera_get_fov`). Base plus the player's offset, clamped to the preset's two
   limits, with the offset rewritten to the stop. The upper test is written `if (!(fov <= max))`, not
   `if (fov > max)`, which is the branch a NaN takes differently — the port keeps the written form.
2. **The target point.** `OffsetFromBodyPoint` turned out of the rider's own axes: `z` along the
   forward row, `y` along the second row, and `x` along their cross product — the cross product being
   skipped entirely when `x` is zero, which it is on every preset the disc ships.
3. **The heading.** The rider's forward row with `y` zeroed and the `xyz` renormalised. `w` comes
   through from the rider's row untouched.
4. **The swing.** `fov_ratio * ((0.21 * CameraSwing) * -lean)`, where `fov_ratio = fov / MaxFOV`.
5. **Forward or reverse** (`camera_forward_reverse`). The arc cosine — through the engine's shared
   table, the same one S11's stick classifier reads — of `normalize(vel) . heading`, with the clamp to
   ±1 written out *twice* in the original. Past a quarter turn and above `ForwardReverseSwitchSpeed`
   it ramps the three reverse offsets up towards the preset's three; otherwise it ramps them down, and
   skips the ramp down entirely once the distance offset has reached zero. The ramps are deliberately
   asymmetric in the original: the look-ahead offset winds *on* at a twentieth of the rate and *off*
   at the full rate.
6. **Aim** (`camera_aim`). Yaw is `AngleY - user yaw - CameraSwing * swing` through `fmodf` and one
   correction into ±pi. Pitch is `user pitch + AngleX - reverse ramp`, clamped to ±1.4835324 (85°),
   with the player's pitch rewritten on a clamp.
7. **The direction.** The heading turned by the pitch about `cross(heading, up)`, and that turned by
   the yaw about up — two calls to the shared axis-angle rotation.
8. **The distance** (inlined into the frame; `Follow::limit_distance` in the port). `reverse ramp +
   (user zoom + FollowingDistance)`, floored by `max(MinFollowingDistance, nearest)` and capped by
   `MaxFollowingDistance`, where `nearest` is the collision-derived distance the viewport keeps at
   `+0x19c`.
9. **The look-at point** (`camera_look_at`). `dot(vel, heading) / speed_reference`, capped at 1, times
   a factor built from the follow distance and the field of view, times `LookAheadDistance`; shortened
   again when the rider is falling faster than 5 or rising faster than 5; replaced outright by the
   ramped reverse look-ahead when the frame is in reverse. The point is `target + normalize(rotate(the
   heading by `swing * (-0.7 * LookSwing)` about up)) * that`, with the height replaced by
   `y + (1 - fov_ratio) * vel.y * 0.25`, the (always zero) ground probe, and the height bias. A
   look-back preset skips the lot and returns `target + -heading * LookAheadDistance`, with no lift.
10. **The position** (`camera_position`). `target + direction * distance`. See the dead swing above.
11. **The filters** (`camera_filter`). Eight one-pole lags: `alpha = min(dt, tau) / tau`, then
    `value += alpha * (target - value)`. A preset with `FilterThisPreset` clear does not
    filter at all; a camera with the snap flag set loads the filters with this frame's values and
    clears the flag. Both vectors come back with `w` of 1.0, because the original rebuilds them a lane
    at a time and puts a literal there.

Then two calls into the viewport — `viewport_set_eye` with the filtered position and the filtered
field of view, and the look-at setter at vtable `+0xc4` of the viewport with the filtered look-at point
and world up — and one into the base class's own update, which has nothing of the camera in it.

Two more routines hang off the player rather than the frame. `camera_set_preset` copies the record
into the object, sets the six vector filters' time constants, arms the snap when moving from an
unfiltered preset to a filtered one, and then deals with the four player nudges (zoom, yaw, pitch,
field of view): switching *to* a look-back preset parks them in a shadow copy, switching *away* from
one takes them back out, and any other switch zeroes them — so a glance behind leaves the camera where
the player had it. `camera_nudge` is the right stick: analogue controls 0x1e and 0x1f are the two
modifiers, and with either held the stick zooms (`* 0.3`) or widens the field of view (`* 0.5`),
otherwise it swings the yaw (`* 0.05`) and the pitch (`* -0.05`), with pitching *down* suppressed while
the camera's pitch-down lock is set.

## How it was proved

Twelve function-oracle proofs in `crates/game-sim/tests/camera.rs`, each over 4000 generated cases
(the rotation and the curve table are their own shape; see below). `./kit check` — 311 tests — passes.

They all work the same way, and the way is the reason they are worth trusting. The harness lays the
original's whole world out in the interpreter's RAM: the camera object, the eight filter structs, an
array of four generated presets, the class's vtable built out of `addr()` lookups rather than copied
off the disc (so a slot that moved is a failed lookup, not a wrong answer), a viewport, the rider's
actor and the pad's control table. It writes a generated `Follow` into that, runs the original, and
snapshots **three whole regions** — the 0x180-byte object, the 0x100 bytes of filters and the 0x80
bytes of scratch the out-parameters live in. Then it writes the same generated state again, applies
the port, writes the port's answer over the top, and compares the three regions word for word, naming
the differing word by region and offset. Comparing regions rather than a list of fields is deliberate:
a field the original writes that the port does not model shows up as a difference, which is exactly
the bug a hand-written list of comparisons hides. On top of that every call is checked for writes
*outside* those regions and its own stack frame — a write to the presets, to the actor or to a global
would be state the port cannot have, and no call makes one.

| Proof | What it covers |
|---|---|
| `the_curve_table_is_the_games_own_to_a_rounding` | the 6000-entry sine/cosine table, against the game's own builder |
| `the_rotation_is_the_games_own` | the shared axis-angle rotation, 4000 cases |
| `the_field_of_view_is_the_games_own` | clamps and the offset rewrite |
| `the_aim_is_the_games_own` | yaw wrap, pitch clamp, both offset rewrites |
| `the_forward_reverse_test_is_the_games_own` | the arc cosine test and all three ramps |
| `the_look_at_point_is_the_games_own` | the look-ahead, the lift, the look-back branch |
| `the_position_is_the_games_own` | the position |
| `the_positions_swing_is_dead_code` | 32 yaws × 200 cameras: the swing changes nothing |
| `the_filters_are_the_games_own` | all eight lags, the snap and the unfiltered preset |
| `switching_preset_is_the_games_own` | the copy, the taus, the snap, parking and unparking the nudges |
| `the_nudge_is_the_games_own` | the right stick, all four modifier combinations |
| `one_frame_is_the_games_own` | `camera_update` end to end, which is also what proves the distance limiting, since the original inlines that and has no routine for it |

Branch coverage, as the generators reach it (each proof asserts its own counts are non-trivial):
896 of 4000 cases in reverse and 1971 ramping down · 2299 filtered, 951 snapped, 750 unfiltered ·
986 preset switches parked the nudges and 1040 took them back out · the four nudge-modifier
combinations came up 1806 / 892 / 877 / 425.

The whole-frame proof needs three hooks, because the frame's answer leaves the original through calls
rather than through memory: `viewport_set_eye` records the position and the field of view off its
arguments, the viewport's look-at slot records the point, and the base class's update is turned into a
no-op.

Measurements worth keeping:

* The game's curve-table builder takes **91,695,962** interpreter instructions, and its index scale is
  954.77045 (6000 entries over a full turn, plus a duplicated 6001st so an index that rounds up off the
  end still reads something).
* **542 of 6000** entries of `CurveTable::build` are bit-exact against it; the worst is entry 269, off
  by 1.1920929e-7 — one `f32` ulp near 1.0. That is why this one proof has a tolerance (1e-6) and why
  `CurveTable::from_entries` exists: every other proof hands the port the game's own table and is
  bit-exact.

Three bugs the proofs caught, all of them operation *grouping*, and all of them invisible to a test
with a tolerance in it:

* `limit_distance` was computing `(reverse + user zoom) + FollowingDistance`; the original's three
  instructions are `max`, then `user zoom + FollowingDistance`, then `reverse + that`. One ulp.
* The look-ahead factor was left-associated as Ghidra prints it; the real order is
  `min(1, along) * (pull * ((1.8 * (1 - d/600)) * pull))`.
* The look swing's angle was `(swing * LookSwing) * -0.7`; the two `mul.s` are really
  `-0.7 * LookSwing` and then `swing * that`. This one is the best argument in the task for a
  bit-exact port: the rotation it feeds puts the swung heading's `z` within a thousandth of zero on
  some inputs, so a last-bit difference in the angle came out as **37 ulps** in the look-at point's
  `z`. A tolerance tuned to the tables would have passed it.

Also fixed: three hand-written compare-and-select helpers (`floor_zero`, `min_of`, `max_of`) were
replaced by `F::min`/`F::max` in the original's operand order, because the original really does use
`min.s`/`max.s`, and those compare bit patterns as signed integers — so `±0` resolves differently from
a compare and a select.

One correction to `research/oracle.txt` that is not this task's: **`sin_double` and `cos_double` were
named the wrong way round**, and had been since whoever added them. Settled by calling both with a
zero argument — 0x00409a50 answers `0x3ff0000000000000` (1.0) and so is the cosine, 0x0040a298 answers
zero — and confirmed by the fact that the curve table's builder stores 0x00409a50's result in each
entry's *first* word, which the rotation reads as the cosine. No shipped code used either name. The
names are swapped and the way it was settled is written into the file.

## Not verified / not 1:1

* **`CurveTable::build` is not bit-exact.** The game builds the 6000-entry table with the SDK's
  *double*-precision `cos` and `sin` (reached through its float↔double converters) and rounds each
  result to `f32`; `ps2-float` has no double-precision transcendentals, so the port uses the
  single-precision ones. 542 of 6000 entries match exactly, the worst is off by 1.1920929e-7, and the
  proof's tolerance is 1e-6. Fixing it means `sin(f64)`/`cos(f64)` in `ps2_float::libm`, the way S11
  added `acos(f64)`. Everything downstream of the table is bit-exact, because
  `CurveTable::from_entries` lets a proof hand the port the game's own table.
* **Nothing was found that writes `speed_reference`.** The constructor sets it to 1.0 and no writer
  turned up in the image, so in practice the look-ahead's speed ratio saturates at its cap of 1 for
  anything above walking pace. The port keeps it as a field and the proofs vary it, so if a writer
  does exist the port already follows it.
* **`LowSpeedAngleXVelCutoff` and `LowSpeedAngleXScaleFactor` have no reader.** Both keys are read out
  of the file into the record (and the port carries them), but no instruction in the camera touches
  either offset. Either a derived class reads them or they are vestigial; I did not find out which.
* **The dead swing in `camera_position` is not ported.** Shown dead by `the_positions_swing_is_dead_code`
  rather than by reading every path, so "dead" here means "cannot change the returned position over
  6400 generated inputs", not "proved unreachable".
* **The camera's collision push-out is not ported.** `Follow::update` takes the nearest allowed
  distance as an argument instead. The push-out is at 0x001e8850 with its helpers at 0x00119be0 and
  0x0011a150; it belongs with whatever task ports the viewport, and it reads the collision world,
  which this crate has no access to from here.
* **The camera file's reader is not ported.** Vtable slot `+0x128` (`camera_load_presets`, 0x001fbf50)
  fills the records from `cpresets.ini`; `game-render`'s `scene.rs` already reads six of the keys for
  drawing, and the full reader belongs with whatever task owns the ini loader. The field offsets and
  every key name are in `research/oracle.txt` so that it is a transcription when somebody gets to it.
* **Derived classes are not ported.** Slots `+0x108` and `+0x10c` are stubs *in this class*; another
  camera class may override them, which would make the frame transform and the ground probe live. I
  did not enumerate the other classes in the vtable neighbourhood.
* **One arm of the reverse test cannot fire.** The window's upper bound is three quarters of a turn,
  which an arc cosine of a value in ±1 can never return. The original is written as if the angle were
  signed; the port keeps the unreachable comparison as written rather than simplifying it, so it is
  "1:1 with code that cannot run".
* **The pitch anti-windup is asymmetric and reproduced as written.** The two rewrites of the player's
  pitch use the reverse ramp with *opposite* signs, which cannot both be right. Only the upper one is
  reachable while the ramp ramps up from zero. Left as the original writes it.
* **`limit_distance` has no proof of its own.** The original inlines it into the frame, so it is proved
  only through `one_frame_is_the_games_own` — 4000 whole frames, but with the frame's own distribution
  of inputs rather than a generator aimed at its branches.
* **`Preset::name` and `Preset::cycle_thru_this_preset` are carried but not exercised.** Nothing in the
  camera reads either; the name is the file reader's, and the cycle flag belongs to whatever front-end
  code steps through the presets, which was not found.
* **Nothing was checked against a screenshot or a recording.** The proofs are against the original's
  own instructions, which is stronger for the arithmetic — but no frame of this camera has been looked
  at next to the real game, and the app layer that would feed it a rider does not exist yet.

## Next

Nothing is outstanding for this task. For whoever picks the camera up again, in the order they would
hurt: `sin(f64)`/`cos(f64)` in `ps2-float` to close the curve table; the viewport and its collision
push-out, which is the one input `Follow::update` currently takes on trust; and the `cpresets.ini`
reader, which is a transcription from the key list in `research/oracle.txt`.
