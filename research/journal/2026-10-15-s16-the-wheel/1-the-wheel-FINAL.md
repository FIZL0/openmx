# 1 — the wheel (FINAL)

Task S16. 2026-10-15.

## What I did

Wrote `crates/game-sim/src/wheel.rs`, the three steps the bike's ground frame drives each wheel
through, in the order it calls them, and proved each one against the original over ten thousand
generated wheels:

| step | original | proof |
|---|---|---|
| `suspension` | `wheel_suspension` | `the_suspension_ray_matches_the_original` |
| `tire_forces` | `wheel_tire_forces` | `the_tires_forces_match_the_original` |
| `integrate` | `wheel_integrate` | `the_wheels_own_integration_matches_the_original` |

The ten fields `wheel_clear` empties — the seven words and three quadwords `move.rs` keeps by offset
in `Fresh`, because S3 did not port what writes them — are named in the module's own table:
`+0xd0 ray_hit`, `+0xd4 swept_contact`, `+0x1f0 hit_owner`, `+0x1f4 hit_vehicle`,
`+0xf0 compression`, `+0xf8 spring_force`, `+0x1a0 velocity_done`, `+0x170 contact_velocity`,
`+0x1b0 other_velocity`, `+0x1c0 other_point_velocity`. Two of the "scalars" are object pointers and
two more are flags, which is why they are an `i32` on this side and an `F` on that one.

`research/oracle.txt` gained the S16 section: the three steps' callees, the five one-line VU0 helpers
the tire forces inline, and the three constant quadwords. `crates/ps2-oracle/src/ee.rs` gained two
MMI arms (`pextuw`, `pexcw`) that the interpreter was missing — the suspension's transpose idiom runs
through them, and without them the oracle aborted.

## What the original does

**`wheel_suspension`** — the ray that looks for the ground *under* the wheel. It starts at the mount
and reaches `+0x214` **along the ground's own normal** (`+0x200`), not straight down and not along the
bike's suspension axis. The direction is the last normal brought back into the chassis' frame,
flattened into that frame's `yz` plane and normalised, so a wheel never probes sideways. Since the
direction depends on the answer, the ray asks again until two answers agree to within five degrees,
at most five times a substep; the tolerance is not a cosine but *one entry of the engine's shared
`{cos, sin}` table*, taken at the index five degrees lands on, so the table has to exist before the
function runs at all.

Then the argument between the two contacts a wheel can have — the ray's and the collision sweep's
(`+0xd4`). The terrain takes over if the sweep's normal leans more than 45° off the suspension
(`0.707`), or if it is a tenth of a unit higher than the contact the wheel is riding, with the same
tenth of slack the other way so the choice cannot flutter. `+0xd8` remembers which side won and is
*not* cleared per substep.

**`wheel_tire_forces`** — the compression, and the spring and damper force out of it. The compression
is geometry, not a distance along the probe: the suspension line runs from the probe's end back up to
the mount, and the compression is how far along that line the contact's *plane* is
(`along = (contact − probe_end)·n / (down·n)`, the point at `+0x140`, the length of the arm). A
contact whose normal — in the chassis' frame, with the chassis' `z` lane thrown away — leans away
from the chassis' own up carries less of the load: the engine's generic remap, inlined, from
`0.5..0.64` onto `0.5..1.0`.

Past `+0x48` the suspension has bottomed out, and then the spring is not asked to do the work. The
chassis is shoved directly: the part of its velocity going into the ground comes off `+0x260`, the
same amount comes off its acceleration along its own up (`+0x170`), and whatever of its accumulated
force is still pressing into the ground is cancelled (`+0x130`). The compression is held at the
travel and the contact velocity is asked for again, because the shove changed it.

What comes out, along the contact's normal, is `+0x54`'s spring against the compression less a damper
with a rate for each direction (`+0x58` leaving the surface, `+0x5c` into it) against the closing
speed clamped to `+-0x4c`, never pulling. A steered wheel the bike is balancing on hands that to its
steering hinge; every other wheel puts it into the chassis as a force at the centre of mass and a
torque about it, both in the chassis' frame. The tail: with no contact the force and the contact
velocity are emptied, and letting go is rate limited to `+0x50` a second — taking load up is not.

**`wheel_integrate`** — thirty-two instructions of dispatch: the surface's rolling resistance, the
wheel's own spin, and then the steering, which has three cases. A wheel that does not steer is left
alone; a steered wheel the bike is not balancing on keeps the angle the ground step commanded and has
its rate thrown away; a steered wheel the bike *is* balancing on turns under its own torque —
`+0x6c`'s torque less `+0x70`'s damping, through `+0x78`'s inverse inertia into `+0x8b0`'s rate, into
`+0x8ac`'s angle, held inside `+0x80..+0x7c` with the rate killed dead at either end.

Names settled on the way: `+0x38` is "this wheel is steered" (it has a hinge of its own: axis at
`+0x8c0`, frame at `+0x8d0`, and the rate goes into the chassis' angular velocity at `owner+0x290`),
`+0x3c` is `balancing` — the same word `move.rs` already had from the other side — and the wheel's
owner (`+0x1e4`) is the chassis, offset for offset the same object `move.rs`'s `Chassis` views.

## How it was proved

Three function-oracle tests in `crates/game-sim/tests/wheel.rs`, ten thousand generated wheels each,
built on `tests/move.rs`'s pattern: lay the wheel out in interpreter RAM, write a generated state in,
run the original with every un-ported callee hooked, write the *same* state again, apply the port
over the top, and compare **every word** of the object — all `0x8c0` bytes of the wheel, and for the
tire forces the `0x2a0` bytes of the chassis as well. `written_ranges()` says the original touched
nothing but those objects and its own stack frame. The hooks record an ordered `Vec<Call>` with the
vector each callee was handed, and that list is compared with the one the port returns, so the port's
claim about *what it calls, in what order, with what* is proved and not asserted.

The generator drives the branches rather than waiting for them: the ground query's answers are
scripted so the ray runs one, two … five passes; the contact is placed near the probe's end so the
suspension's geometry comes out on a wheel's scale; the travel is wide enough to clear the bottom-out
in one case in three; a direction is exactly zero one time in thirty-one so the VU's reciprocal
square root of a zero length is proved rather than avoided. Each test ends with a count per branch
and asserts every one was reached — fourteen of them for the tire forces (no contact, the plane
parallel to the suspension line, the plane behind the wheel, a contact inside the travel, bottomed
out with and without the push, both damper directions, the hinge path and the chassis path, a spring
force and one clamped to zero, the load scale strictly between its ends, the extension rate limit).

Two mutations, run to check the proof bites: swapping the two damper rates, and dropping the force
the chassis gets. Both failed at generated state 38 — the first on the call list (the force vector
the hinge is handed), the second on `chassis + 0x130` word for word.

The five-degree tolerance needs the engine's shared `{cos, sin}` table, so the harness builds it with
the game's own builder (`mem_alloc` hooked to put it where the test wants it) and hands the port the
same 6000 entries. With the global still null the tolerance reads garbage below zero and the ray's
loop never runs — which is how that dependency was found.

`./kit check`: 383 tests pass.

## Not verified / not 1:1

- **The callees are hooked, not ported.** `ground_query` (the world's "where is the ground under this
  point", eighty-four callers across the game), `wheel_contact_velocity`, `wheel_surface_drag`,
  `wheel_spin_integrate`, `wheel_hinge_force`, and the chassis transform's four
  (`transform_unrotate`, `transform_rotate`, `transform_unpoint`, `transform_sync`). What is proved
  about each is its place in the order, the object or vector it is handed, and — for the two halves
  of the integration — the `dt` in `f12`. What each one *does* is not ported and not proved. The four
  transform calls all go through the engine's lazy transform update, which may rebuild a chain of
  parent frames and write outside the wheel, which is why they are not inlined even though three of
  them are arithmetic this crate already has.
- **`wheel_contact_velocity`'s hook writes nothing**, so both sides read the generated `+0x170` and
  `+0x180`. That proves the port reads the right offsets and nothing about the callee's own
  arithmetic; in the game those two fields would have been rewritten between the two calls the
  bottom-out path makes, and the proof cannot see the difference.
- **The dead bottom-out push vector.** The original computes `0.9 × (compression − travel)` along the
  contact's normal, stores it in a stack slot and never reads it again — the three pushes that follow
  are what actually move the chassis. The port keeps it as `_push` because it is what the original
  computes, and *nothing in the proof can see it*: neither the `0.9` nor the vector reaches any
  compared word. If that constant matters somewhere, it is not here.
- **The remap's dead arm.** The inlined remap guards against a zero-width input range — "if `lo ==
  hi`, leave the scale at `lo`" — on two constants that are not equal. Unreachable, so not ported.
- **The force path's null-transform early-out.** `owner+0x254 == 0` sends the force application
  straight to the tail; the top of the same function already dereferenced that pointer
  unconditionally, so the branch cannot be reached without the function having crashed first. Not
  ported, and not provable either way without feeding the original a null it cannot survive.
- **One uninitialised register.** `along` is written by a division that two paths skip, and the
  bottom-out test reads it anyway. Reaching that read needs a *negative* travel (`+0x48 < 0`), which
  no wheel is built with and which the generator never produces; the port substitutes zero there.
- **The suspension's zero-iteration edge.** If the tolerance were ever at least one, the ray's loop
  would not run and the original would compare a stack slot nothing wrote. It cannot happen while the
  tolerance is a table entry below one, so the port returns the zeroed point and normal rather than
  modelling a read of uninitialised stack.
- **Inlined instead of called.** Five one-line VU0 helpers (a quadword copy, `vsub.xyz`, an
  add-into-place, a cross product, a component-wise multiply) are written out in the port rather than
  recorded as calls. Moving words about and a masked lane op cannot round differently, and the
  word-for-word comparison covers them — but the call list does not mention them, so a reader
  comparing the port against the disassembly will see five calls on one side and none on the other.
- **The multiply by `(1, 1, 1, 1)`** on the torque's way into the chassis is kept because the
  original has it. A wrong constant there would be caught; "multiplied by one" against "not
  multiplied at all" differs only for denormals and NaNs, which the generator does not produce.
- **Inferred names.** `+0x38` `steered` rests on the dispatcher and on `wheel_spin_integrate` reading
  the hinge's frame, not on a string or a tuning key. `+0xf4` `last_compression`, `+0x4c`
  `max_damper_speed`, `+0x50` `extension_rate` and the two damper rates are named for what the
  arithmetic does with them. `+0x1f0`/`+0x1f4` are named from the contact distributor
  (`wheel_set_contact`), which this task did not port.
- **The flag words are `bool` here.** `has_ground`, `steered`, `balancing` and `contact` are tested
  with `beqz`/`bnez` in the original and only ever poked as 0 or 1 by the harness, so a word holding
  something other than zero or one is untested.
- **Nothing ran in the emulator.** All three steps are proved against the original's own code through
  the interpreter, not against a live frame of the game; S8's acceptance run covers the ground step
  above them. No PCSX2 copy was started for this task.
- **`wheel::Chassis` is a second view of the same object** `move.rs`'s `Chassis` already describes.
  It is kept separate on purpose — the tire forces need `+0x140`, which that one does not carry, and
  another agent is working in `move.rs` this run.
- **The two new MMI arms in `ps2-oracle`** (`pextuw`, `pexcw`) are exercised only by the suspension's
  own proof; they are not covered by a test of their own.

## Next

Nothing is waiting on a human. The wheel's remaining pieces, in the order they would pay off: the
contact velocity (`wheel_contact_velocity`, which owns four of the ten `Fresh` fields), the surface's
rolling resistance and the wheel's spin (`wheel_surface_drag`, `wheel_spin_integrate` — the two the
integration records), the steering hinge's force application (`wheel_hinge_force`), and the contact
distributor (`wheel_set_contact`) that fills `+0xd4`, `+0x1f0` and `+0x1f4`.
