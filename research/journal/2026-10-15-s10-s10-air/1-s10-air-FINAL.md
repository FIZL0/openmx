# 1 — s10 air (FINAL)

Task S10. 2026-10-15.

## What I did

`crates/game-sim/src/air.rs`: the five steps the bike's update runs for a jump, ported bit-exact. One new
module, one new line in `lib.rs`, one new integration test file, one line in the `PLAN.md` code map, and
the S10 block at the end of `research/oracle.txt`. Nothing existing moved, renamed or reformatted.

The module is five public functions, in the order the original calls them, because each reads flags the
one before it wrote:

1. `contact` — which wheels touch, how long since they left the ground, the velocity at the contact
   patch, and the *crossup*: the signed angle between where the bike points and where it is going.
2. `preload` — stick back then forward on a lip: a force into the chassis, repeated every substep for
   `PreloadTime`.
3. `backflip` — stick forward then back on a lip: a pitch torque that decays while the stick holds it.
4. `airborne_control` — the moment the stick commands in the air, damped or auto-levelled, and the lean
   force that drifts the bike sideways.
5. `wreck_test` — the g loads, the pitch and roll limits, the landing crossup test, the no-hander and
   the joust.

The state is split four ways, so that a reader can see at a glance that no step writes anything outside
`Air` and the chassis' two accumulators: `Params` (what the bike's `.phy` file sets, never written),
`Air` (what the air code owns), `Chassis` (the rigid body's force, moment, velocities and angles), and
`Step`/`Loads` (what the rest of the update hands in). `Wreck` carries the original's own wreck ids
(1 front, 2 left, 3 right, 4 back, 5 flat, 7 joust, 8 nose dive, 0xb tail, 0xc no-hander), because the
animation, the camera and the respawn all switch on them.

Two numbers the air code reads out of RAM had to come with it:

- `AcosTable` — the 30000-entry arc cosine table the game builds at startup and interpolates for the
  crossup angle, with the last entry duplicated so a lookup at `x = 1` can read `table[i + 1]`. The game
  fills it by calling its *double-precision* `acos` 30000 times and rounding to `f32`, so the module
  carries that `acos` too: fdlibm's `__ieee754_acos`, coefficients read out of the executable's pool, in
  `f64` — IEEE basic operations and `sqrt` only, so it is host-independent.
- `STEEP_SINE` — the airborne control step asks whether the bike is pointing more up or down than along
  by comparing the heading's `y` against the sine of 70 degrees, which it looks up in the startup
  `{cos, sin}` table every substep. The index is made of nothing but startup constants, so the port
  folds the whole lookup into one constant: `0.93934375`.

## What the original does

Recipes: `./kit fn 00251d20` (contact), `00256940` (preload), `00256b50` (backflip), `00256e50` (airborne
control), `002522c0` (wreck test); `0012e150` and `0012e260` build the two tables; `0040aba8` is the
double-precision `acos`, `00418700` the double-to-float conversion. All of them are in
`research/oracle.txt` with their argument lists.

**Contact.** `on_ground` is either wheel's contact flag, `both_wheels` is both, `airborne` is neither;
`was_airborne` is last substep's answer, kept so the landing test can see the transition. The hang time
resets on the way up and accumulates `dt` while airborne. The contact velocity is the front wheel's, or
the rear's, or — with both wheels off the ground — the chassis' own local velocity, which is what makes
the lip test work in the air as well as on it.

The crossup is the interesting part. It is not a dot product: the heading and the velocity are both
flattened into the horizontal plane and normalised (VU0's `vrsqrt`, the approximate one), crossed, and
the *length* of the cross product is taken through the FPU accumulator in x, y, z order — that is the
sine of the angle. The table gives `acos(sin)`, and `pi/2` minus that is the arc sine, which is signed
and bounded at a quarter turn either way; the sign is the cross product's `y`, which is which way round
the bike is sideways. It is measured only when the bike is not mid-backflip and faster than 10, and while
control is ramping back in after a flip the whole angle is scaled by the ramp.

**Preload and backflip** share one gate, which I called `on_a_lip`: a wheel on the ground, the chassis
already rising faster than 5, the contact patch moving forward faster than 30, and no brake. Both arm on
one stick direction and fire on the other — the preload back then forward, the backflip forward then back
— and both turn how long the stick was held into a magnitude through the same inlined "map and clamp"
over a Min/Max pair of times. The preload has a second pair for the d-pad, which cannot be held half way.
An empty range (Min equal to Max) gives the preload nothing: the register the result comes out of is
zeroed on the way into the function, which the decompilation shows as an assignment inside the lip test's
condition chain. The backflip's empty range gives 1.25 times the minimum torque, because that is what its
register already held.

A fired preload reloads the timer with `PreloadTime` and adds the same force to the chassis every substep
until it runs out, which is why a preload is a shove and not an impulse. A triggered backflip has 0.6 s
to get the wheels off the ground or it is cancelled; while it turns, the torque decays at one rate when
the stick pulls and another when it pushes, and stops decaying entirely while the d-pad is asking for a
trick. It ends when a wheel touches, when the bike hits something, or when the torque has decayed past
the minimum — and then the control-authority ramp starts, either because the bike is down or because it
is the right way up, which is how a flip that comes round cleanly gives the stick back sooner than one
that does not.

**Airborne control** is the step that makes the air feel like it does. The stick commands a moment
straight out of the bike's `AirborneControlMoment`, with the pitch axis on the stick's `y` and the roll
and yaw axes both on its `x`; in a stunt the three are scaled by 0.5, 0.4 and 0.4. Mid-flip the roll and
yaw components are zeroed, and the whole thing is scaled by the control-authority ramp. Then one of two
things happens. If the bike is steeply pitched, or the rider is leaning, or it is mid-flip, or control has
not come back yet, the moment is damped: minus `AirborneDamping` times the whole angular velocity. If none
of those, it is *auto-levelled* instead — the yaw axis is driven towards `UnyawGain` times the crossup and
the roll axis towards `UnrollGain` times the roll angle, each through its own torque, and the pitch and
yaw rates are damped only outside a 0.1 dead zone. Braking in the air adds `BrakeGyroTorque` to the pitch
axis, the way a real wheel's gyroscope would. Last, with a wheel's worth of speed and no flip torque in
flight, the rider's lean pushes the chassis sideways: up crossed with the velocity, normalised, times
`AirborneControlForce` times the lean.

**The wreck test** is a chain of tests, first match wins, and the match is reported by calling the bike's
"the rider came off" vtable slot with the wreck's id. In order: the three g loads against their four
limits (back, front, side, and a vertical limit that is one of three numbers depending on no-hander,
racing or freestyle); pitch past either ground limit while a wheel is down; pitch or roll past either
airborne limit while airborne and not flipping; a flip torque still being applied when the bike touches
something; and, on the substep the bike lands after long enough in the air, the landing crossup outside
its window — left or right depending on which side of a half turn it came down on. Then the trick system's
own test: with the pose not yet where it was asked to be, a no-hander whose up axis has rolled too far is
a no-hander wreck, and anything else with a settled pose is a joust, which writes the impact velocity the
camera and the ragdoll use.

## How it was proved

`crates/game-sim/tests/air.rs`, seven tests, `./kit check` 146 tests passing.

**The two tables.** The arc cosine table is compared entry for entry, by bits, against the one the game's
own startup code builds — all 30001 of them, with `mem_alloc` hooked to hand the builder a fixed buffer
because an oracle run starts with an empty heap. It takes the interpreter 268,331,996 instructions. The
steep-lean sine is proved by building the startup `{cos, sin}` table with the game's builder and indexing
it the original's way (wrap the angle, add `pi`, scale by the index scale at `0x0046a698`, truncate: entry
4165), then comparing with the port's folded constant.

**The five steps,** each over 8000 generated states — 40,000 in all. The harness lays the whole object
graph out in the interpreter's RAM (the bike, its chassis, its two wheels, its collision record and a
vtable), writes a generated state into it, and runs the original. Then it zeroes the regions, writes the
*same* state again, applies the port, writes the port's answer over the top, and compares the whole region
word for word — 4.4 KB of it, not a hand-written list of fields. That is the point: a field the original
writes that the port does not model is a difference, which a list of comparisons would hide. Two more
checks ride along: every write the original makes has to land inside the object graph or the call's own
stack frame (a write to a global would be state the port cannot have), and the wreck test's answer, which
is a call and not a write, is caught by a hook on the vtable slot and compared with the port's
`Option<Wreck>`.

The generator is biased, because the interesting paths are four conditions deep: a third of the states are
on a lip, the ground and the air get half the states each, the stick lands exactly on the ±0.8 threshold a
third of the time, a tuning range is empty one time in eight, and the wreck limits are drawn from the same
range as the loads and angles they are compared against. Every proof tallies which path each state took
and asserts the tally, because a proof is only worth the branches its states reach:

```text
contact           one wheel 4001 · both wheels 2044 · airborne 309 · mid flip 947 · slow 334 ·
                  control coming back 365
preload           off the lip 3736 · waiting 740 · arming 167 · held 587 · firing 91 · shoving 2450 ·
                  spent 229
backflip          off the lip 3684 · waiting 1308 · arming 155 · triggered 92 · in the grace 684 ·
                  off the lip in time 681 · turning 304 · done turning 1092
airborne control  on the ground 3923 · steeply leant 591 · leaning 1750 · no control yet 1324 ·
                  unyawed 211 · unyawed in a stunt 201
wreck test        no wreck 274 · front 2276 · left 1018 · right 1038 · back 1941 · flat 567 ·
                  joust 54 · nose dive 309 · tail 402 · no-hander 121
```

Three things the proofs caught that reading the decompilation had not:

- The torque decay is `torque − dt × (stick × rate)`, not `torque − (dt × stick) × rate`. One ulp, in the
  one place a backflip's whole feel comes from.
- The game's `double_to_float` **chops** towards zero rather than rounding to nearest: 18595 of the 30001
  table entries were one ulp high until the port's narrowing did the same.
- fdlibm's `acos` for `x >= 0.5` returns `2 × (df + w)`, built on the truncated `df` and not on `s`. With
  the sum built on `s` instead, 6992 entries — every one with `x >= 0.5` — came out wrong.

Two of the three are last-bit differences that no screenshot and no eyeball would ever have found, which
is the argument for the byte comparison in one sentence.

## Not verified / not 1:1

- **The wreck test's prologue is cut.** The real function starts with an out-of-bounds and hitzone test
  (`bike + 0x570`, which the harness forces to zero) before the chain this task ports. It belongs with the
  track and the collision work, not here, and nothing in `air.rs` models it.
- **The wreck test's bike-against-bike loop is cut.** The harness hands it an empty bike list, so the
  joust-against-another-rider half is neither ported nor proved. The `Joust` code is still reached, through
  the landing-pose path, and is proved there.
- **The heading and up axes are inputs.** The original samples them out of the chassis' orientation matrix
  at the top of the contact step; the proof hooks that function (`./kit fn 001b24d0`) to a no-op and writes
  the two axes itself, so the sampling is not proved here. It is another task's function and is written
  down in `research/oracle.txt`.
- **The three g loads and the landing crossup are inputs.** `Loads::side`, `vertical`, `fore_aft` and
  `landing_crossup` are computed by the suspension and landing code, not by anything in this module.
- **The trick system's fields are read, never written.** `pose`, `pose_wanted`, `pose_blend`, `stunt`,
  `trick_wrecked` and `no_hander` come from S11, and the last branch of the wreck test is really the trick
  system's, reached from here. The backflip overlaps the trick system too: what the d-pad asking for a
  trick means is S11's, and all this module knows is that it stops the torque decaying.
- **The wreck-enabled global and the already-wrecked early return are the caller's.** The original checks
  a global (`0x00420bc0`) and the bike's own wreck state before the chain; the port has neither, because a
  module with hidden state is the thing the rules forbid. The caller gates it.
- **`STEEP_SINE` is a folded lookup, not a lookup.** It is proved equal to the entry the game's own table
  build puts there, and the curve table is rebuilt by the game's builder inside the harness, so all 40,000
  states compare against the real entry — but the port holds a constant, so a different table build would
  not follow it.
- **The double-precision `acos` lives in `air.rs`.** It belongs in `ps2-float` beside the other
  transcendentals; it is here so that this task's diff touches no shared file while other tasks are in
  flight. Moving it is a one-commit job for whoever needs `acos` next.
- **One substep at a time.** Every proof runs one call of one step against one generated state. The five
  are not proved as a sequence, and nothing here is proved against a real tick of the game — that needs the
  whole bike update and a replay fixture, which is a later task's.
- **The wreck callback's own work is out of scope.** The hook returns immediately after noting the id, so
  what the game does with a wreck (the animation, the camera, the respawn) is neither modelled nor proved.
- `tests/diag.rs`, the temporary hook-and-print test used to find the two `acos` bugs, was deleted once it
  had done its job.

## Next

Nothing is pending for this task. For whoever picks up the air code next: the three cut pieces above are
each a small, well-defined job once their own task lands (the hitzone prologue with the collision work, the
bike-against-bike joust with multiplayer bikes, the orientation sampling with the chassis), and the
double `acos` should move to `ps2-float` the next time anything else needs it.
