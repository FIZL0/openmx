# 1 — s19 chassis (FINAL)

Task S19. 2026-10-16.

## What I did

- New module `crates/game-sim/src/chassis.rs`: the rigid body the ground step pushes on. Its own
  integrate virtual (the whole of it), the per-frame measure that gives `Chassis::speed` its value,
  the transform's `rotate`/`unrotate` pair, the two-step integrator the body reaches through a
  function pointer on itself, `chassis_set_flags`, and `bike_wheel_contacts` with the per-wheel
  contact write it calls. Float lint on, every operation through `ps2-float`'s `F`/`Vf`/`Acc`/`vu`.
- New proof `crates/game-sim/tests/chassis.rs`: seven tests, all against the original in the oracle.
  The two whole-object ones compare **all 0x330 bytes** of the body, word for word, and the ordered
  list of what the step called with the vectors it handed over; the distributor's compares the bike's
  `+0x7d4` and both wheels whole.
- Taught the interpreter two more MMI ops (`pceqw`, `ppach`) in `crates/ps2-oracle/src/ee.rs`: the
  step's correction block is gated by a branchless quadword compare built out of them, so without
  them the integrate cannot be run at all.
- Recorded the addresses and the body's layout in `research/oracle.txt` (the kit's `[symbols]` is
  git-ignored, so that file is the committed copy the proofs resolve names from), and a code-map line
  in `PLAN.md`.

## What the original does

The ground step (`move.rs`, S3) spends a substep filling three accumulators on this object — a force
at `+0x130`, a torque at `+0x140`, a position impulse at `+0x150` — and then calls one virtual on it,
`vtable+0x3c`, `./kit fn 0x15c540`. That call is most of this module. In order:

1. **Asleep?** `+0x1b4` zero and it returns at once — but *after* emptying the three accumulators,
   the correction axis at `+0x160` and the contact count. A force is good for exactly one substep
   whether the body moved or not.
2. **Where the mass is**: `+0x190` (the centre of gravity in the body's frame) through the transform
   at `+0x254` into `+0x60`.
3. **The transform's own turn, carried onto the body.** The transform's rotation (`xform+0x80`, three
   rows) is read back as a quaternion, and `delta = matrix ⊗ conj(body)`; if `delta.w < 0` it is
   negated — the same rotation the short way round — and `body = normalise(delta ⊗ body)`. This is how
   a parent frame or a bone moving the transform moves the body.
4. **Force and torque spent**: `accel = force * (1/mass)` (`+0x214`), `ang_accel = inv_inertia *
   torque` — the inertia tensor at `+0xf0`..`+0x110` goes through exactly the three instructions a
   transform rotates a vector with.
5. **Four integrations**, all through the function pointer at `+0x25c` (`./kit fn 0x13e460`, two-step
   Adams–Bashforth, `x += (dt/2)(3 dx − prev)` then `prev = dx`, with `prev` living at `dx + 4n`):
   acceleration into velocity (n=3), velocity into position (n=3), angular acceleration into angular
   velocity (n=3), and the orientation's derivative into the orientation (n=4, then renormalised).
6. **The limits.** A maximum speed² at `+0x220` and a maximum angular speed² at `+0x224`, each
   applied as `v *= sqrt(limit / len²)` when `len² > limit` — FPU `mula.s`/`madda.s`/`madd.s`,
   `div.s`, `sqrt.s`, not the VU. Then a per-axis window on the angular velocity (`+0x230` maxima,
   `+0x240` minima) whose two tests are deliberately not the same shape: `w <= max` and then
   `w < min`, so a value exactly on either bound is left alone.
7. **The six freedom flags**, `+0x2a0`.. linear and `+0x2ac`.. angular. A zero pins one axis, and it
   clears three words, not one: the velocity lane, the acceleration lane, *and* the integrator's
   `prev` for that lane (`+0x2dc`/`+0x31c` + 4i). The clear happens after the acceleration is
   computed, not instead of it.
8. **The correction the contacts asked for**, if any. The test is a branchless quadword compare of
   `+0x160` against the constant at `0x426d70` — `(0, 0, 0, 1)` — with `pceqw`/`ppach`/`xor -1`/`movz`,
   so *any* word differing counts, down to a negative zero. The axis' length is the angle, the axis is
   normalised with a VU `vdiv`, turned into the body's frame by a ten-operation double cross product,
   made into a quaternion and multiplied onto the orientation.
9. **The transform written back**: origin = where the mass now is, less the centre of gravity rotated
   into the world, then the "frames below me are stale" flag set.
10. **The two measures the next frame reads**, each through a one-line virtual on the body:
    `vtable+0xb4` (`0x15cbf0`) is the velocity unrotated into the body's own frame, `+0xc0`
    (`0x15cbe0`) the angular velocity rotated into the world.
11. And then the same clear as the sleeping path.

The per-frame measure, `vtable+0x38` (`./kit fn 0x15c4c0`), is two numbers: the orientation as three
angles into `+0x90` — copied as a whole quadword, so the fourth lane lands too — and `|velocity|`
into `+0x34`, FPU again, then the base class' own per-frame update.

`./kit fn 0x1b2ff0` and `0x1b30e0` are the transform's rotate and unrotate. The rotate is three VU0
instructions over the rows at `+0x80` with `w` coming through from the vector. The unrotate transposes
first, with five MMI packs (`pextlw`, `pexcw`, `pextuw`, `pcpyld`, `pcpyud`) — and then a sixth
instruction that is not an MMI one at all: a plain `move`, which is a 64-bit `or` and writes only the
low half of its destination. That is what finishes the third column: the pack supplies `x` and `y`,
and the register it is moved into *already held row 2*, so `z` is row2's own `z`. Read as a quadword
copy — which is how it reads at a glance — the third column's `z` comes out as row0's `w`, and the
unrotate looks like it has a bug in it. It does not. Ten thousand generated row sets through the
original settled it; the first version of this port had the wrong one and the first test case failed.

The contact distributor, `./kit fn 0x257820`, is the one call in the ground step's substep loop handed
the *frame's* `dt` rather than the substep's — and it never reads it. It clears the bike's rear node
word (`+0x7d4`), empties the collision record through a virtual at `*(search+0x38) + 0xa8`, runs the
sweep (`0x11a9d0`), and then — only if bit 5 of the byte at `search+0x116` is set — walks the record's
four parallel arrays (`+0x90` stride 0x20 and its `+8` the selector, `+0x94` normals, `+0x98` points,
`+0x9c` nodes, count at `+0x8c`). Per contact: every listener of the surface's owner is told it has
been touched (before either wheel is written); a vehicle counts as one only if `*(vehicle+0x1b0) == 2`;
and a selector *above zero* goes to the front wheel, anything else — zero included — to the rear,
whose node is kept on the bike. The per-wheel write (`0x25b250`) is two quadwords and three words.

## How it was proved

Function oracle, `crates/game-sim/tests/chassis.rs`, seven tests, all passing (`./kit check`: 391
tests across the workspace):

- `the_transforms_rotate_and_unrotate` — 10,000 generated row sets and vectors through each of the two
  real functions, the out-parameter compared word for word. This is the test that decides the third
  column.
- `the_two_step_integrator` — 2000 cases over `n` = 0..16, both arrays compared whole. That covers the
  plain loop (`n < 9`), the eight-at-a-time unrolled path above it (which the chassis never reaches)
  and the `n <= 0` early return.
- `the_freedom_flags` — 2000 bodies through `chassis_set_flags`, all 0x330 bytes compared, so "it
  writes three words and nothing else" is part of the proof.
- `the_vtable_is_what_this_module_ports` — the five vtable entries this module was written from, read
  out of the image, plus the two instructions of the contact-count virtual the port inlines.
- `the_measure` — 10,000 bodies, all 0x330 bytes, and the two calls it makes with the `dt` it passes on.
- `the_integrate` — 10,000 bodies, all 0x330 bytes, `written_ranges()` checked so a write anywhere
  outside the body fails the test, and the full ordered call list with every vector in it. Twelve
  branch counters are asserted non-zero: the sleeping path, both signs of the orientation difference,
  a linear axis pinned, an angular axis pinned, each speed limit reached, each end of the per-axis
  window, a correction asked for, none asked for, and a correction whose axis has no length at all
  (the VU divide by zero).
- `the_contact_distributor` — 10,000 generated frames of 0..5 contacts, the bike and both wheels
  compared whole, the ordered call list compared, and nine branch counters asserted: the flag clear,
  no contacts, front, rear, exactly on the boundary, an unowned surface, an owner with listeners, and
  a vehicle of the kind that counts and of a kind that does not.

Every callee this module does not port is hooked, and each hook does two jobs: it serves the answer
the generator chose (so every branch below it can be driven) and it records what it was handed, which
is the port's own arithmetic. The bodies dispatch through the **real** vtable with the **real**
integrator address in `+0x25c`, and the two one-line virtuals and the integrator run as the original's
own code, not as stand-ins.

## Not verified / not 1:1

- **The transform's and the maths module's own arithmetic is hooked, not ported.** Six callees:
  a point through a transform (`0x1b3060`), a matrix to a quaternion (`0x13f7b0`), a quaternion
  derivative (`0x13ec40`), a quaternion from an axis and an angle (`0x13ec80`), the orientation
  written onto a transform (`0x1b2130`) and the stale flag (`0x1b4ba0`). What the step *does* with
  their answers is proved over generated answers; what they compute is not this task's. `integrate`
  returns them as an ordered `Vec<Call>` instead, the same seam S3 used.
- **`Chassis::speed`'s angles come from a hook** (`quat_to_euler`, `0x13fa60`), as does the base
  class' per-frame update (`0x133500`) — the measure's proof is that it copies the whole quadword into
  `+0x90` and passes the frame's `dt` on unchanged, not what either callee computes.
- **The contact-count virtual is inlined.** `vtable+0xa4` is one store (`sw $zero, 0x1e0($a0)`); the
  port writes the field directly. The test checks those two instructions are still what they were, but
  a *derived* class with its own override would not be modelled. No derived class is ported yet.
- **The port assumes `+0x25c` holds the two-step integrator.** The rigid body's constructor
  (`0x158ce0`) writes `0x13e460` there and nothing else in the game writes it; the proof installs that
  address rather than deriving it, so a body built some other way is out of scope.
- **The distributor re-reads its listener count every iteration** (`*(holder+0xf4)` is the `while`
  condition) while the port takes the count once. Only a listener that added or removed listeners
  while being notified could tell the difference; the generated cases cannot, because the hook that
  answers the notification does not touch memory.
- **The distributor's two indirect calls go through harness addresses.** The record's clear
  (`*(search+0x38) + 0xa8`) and the listener's notification (`*(*element) + 0xc`) are function
  pointers in data; the test puts its own addresses there and hooks those. That proves the *slots* the
  original reads and the order and arguments of the calls, not the bodies at the other end.
- **`bike_wheel_contacts`' `dt` is unread, and the port takes it only to say so.** That it is the
  frame's `dt` and not the substep's is S3's finding, not re-proved here.
- Nothing in this module was run against the emulator: it is reached only through the ground step,
  which P2 wires up. The proofs are the oracle's.

## Next

Nothing outstanding for S19 itself. For whoever wires the ground step to it (P2): `integrate`'s
`Given` is the six hooked answers in one struct, so the caller has to supply the transform's own
arithmetic — a quaternion from a matrix, a quaternion derivative, a quaternion from an axis and an
angle, and a point and two vectors through a transform — before a body can take a real step. Those are
the next functions worth porting, and the `Call` list says exactly which arguments each one gets.
