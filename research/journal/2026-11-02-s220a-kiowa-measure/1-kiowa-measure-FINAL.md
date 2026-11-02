# 1 — kiowa measure (FINAL)

Task S220a. 2026-11-02.

## What I did

- `crates/game-sim/src/kiowa.rs`, appended after S220:
  - `Measure` (`kiowa_measure`'s figures), with:
    - `Measure::flight(body, acos_table)`.
    - `Measure::touchdown(touching, entries) -> bool`. It returns whether the original would play its
      touchdown sound, if the helicopter has one.
    - `Measure::readings(flown, touching, gravity_scale) -> Readings`.
  - `Touch`: one collision-record entry as the touchdown reads it.
  - `Crash`, `CrashContact`, `Strike` and `Crashed`, with `Crash::step` (`kiowa_wreck_check`). The
    rotor's collision is passed in as a closure, called only while the helicopter is still flying, as
    the original does.
  - `rotor_in_ground(matrix, centres, ground)`: the rotor spheres against the ground, in the original's
    accumulation order.
  - `spin_blades(angle, hidden, dt)` (`kiowa_blade_spin`).
- It reuses the biplane's `flow_angle`, `Area`, `CRASHED`, `THROWN` and `EDGE`, the math module's
  `quat_to_euler` and `quat_to_mat4`, and the contact module's `unturn`, `minus`, `scaled` and `dot`.
- Corrected the docs of `Flight::pilot_flag` and the module's list. The field keeps its name.
- `research/oracle.txt`: an S220a block naming the measure, the crash test, the MX measure, the blade
  spin and three scene-node functions the spin calls. It also maps the measure's, crash test's and
  collision record's fields and the vtable slots.

## What the original does

- **Measure** (vt+0x1ac, base). In order:
  - The body wrapper's measure.
  - speed = sqrt(mula/madda/madd of the world velocity).
  - The body-frame velocity, with the same VU sequence as the biplane's `unturn`.
  - The world rates, by `chassis::turn`.
  - alpha = flow angle of (z, y) and beta = flow angle of (z, x): the same inline code as the
    biplane's, the cosine clamped twice and the acos table interpolated.
  - beta is then held to ±pi (max.s, min.s). This is bit-identical in practice, because the table
    never gives more.
  - The beta hysteresis. Take last frame's beta and the new one. If their signs (as ±1, multiplied
    as integers, then converted) differ, |new| > pi/2, and ||new| − |old|| < 0.1745329 (10°), last
    frame's beta is kept. A helicopter flying backwards whose sideslip wraps from +pi to −pi keeps
    the old sign.
  - The Euler angles (`quat_to_euler`), and the matrix (`vu_quat_to_mat3` on the quaternion and the
    origin pos − turn(q, cofg)). The forward row (2) and the up row (1) are copied out, xyz only.
  - The crash test, vt+0x1b4.
  - The touchdown. If the last touching flag (+0x420) is clear and the collision record is touching,
    the entries are searched for the first of kind 0 with +0x24 < 1. If one is found and the
    helicopter has a sound (+0x54c), `touchdown_sound` and `sound_trigger` play it. The search stops
    at that entry either way.
  - +0x420 = the touching flag.
- **Crash test** (vt+0x1b4, the same function on both vtables).
  - Edge throw. Unless reason is 13, `position_query(150, scene, pos)` reports the body within 150 ft
    of the terrain's edge (the biplane's `Area::outside`). The throw is then the biplane's with the
    Kiowa's constants:
    - fall 9.316771, cap 1843.2, up 16.1, lift 150.
    - the speed toward (w/2, y, d/2) is min(cap, w/2)/k, with k = 2·sqrt(fall).
    - velocity.y += 16.1k + 150/k.
    - +0x194 is halved. The unwreck doubles it back; its reader is not known.
    - three rand draws for spins (0.5, 0.4, 0.5)·unit, and rates.w = 1.
    - then wreck(13).
  - If the reason (re-read) is 0, three crash conditions:
    - A rotor sphere in the ground. With the rotor's collision object (+0x458) and a world (+0x150),
      each of its 8 spheres' centres is put through the measure's matrix: row3 + 0, then + row0·x,
      + row1·y, + row2·z, xyz only. Each point is handed to `ground_query`, and if the y it comes back
      with is not ≤ the point's, the rotor is in the ground. Every sphere is asked; there is no
      early out.
    - A struck rotor. The rotor's collision is updated (its vt+0xac against the matrix, then
      `collide_against_search`). If its contact bit (+0x116 bit 5) is set, the struck object's flags
      must not meet +0x464.
    - A fast contact: the record is touching, its surface is null or its flags miss +0x460, and the
      speed is not ≤ MinWreckSpeed.

    Any of the three is wreck(1). There is no upside-down test, unlike the plane.
  - Otherwise (wrecked) the clock runs, including on the frame of the throw.
    - Thrown: after 2.5 s and touching, the blades are thrown (vt+0x1b0), the helicopter is down and
      the clock restarts. Once down and past WreckTime, the velocity xyz ×0.3 and the put-back
      (vt+0x198) runs.
    - Crashed: past WreckTime, the put-back.
- **MX measure** (0x003922e0). The base measure, then four calls:
  - `kiowa_blade_spin`. Unless the rotor's node is hidden (byte 3 bit 6), +0xe58 += 27·dt, and the
    main and tail rotor nodes are turned by it. When hidden, the wrecked-blade node is updated
    instead.
  - Three copies into presentation records. 0x00391d50 copies figures into +0x7a4/+0x874/+0x934
    under frame-bit masks; 0x00392010 binds a node table once and reads a node's transform into
    +0xc90; 0x00391f10 snapshots into +0x560..+0x6fc, including (1 − lateral)/2, (long + 1)/2 and
    the speed.

    None of the three feeds the flight, so they are not ported.
- The wreck slot (vt+0x1b8) sets the reason, clears down, and sets +0x3d8. It also sets the body's
  damping, inertia and gravity (−WreckGravity·mass·32.2) and the sound; MX's adds the blade throw
  unless the reason is 13, rumble, and the node swap. The wreck does not reset the clock; the
  unwreck does. These are not ported here (follow-up filed).

## How it was proved

- `crates/game-sim/tests/kiowa.rs` has three new tests on S148's state, all bit-exact.

  | Test | Cases | What it varies and checks |
  | --- | --- | --- |
  | `the_measure_is_the_originals` | 10,000 | Orientation, velocity (a third flying backwards near beta = ±pi, some straight abeam, zero, tiny), rates with w, position, cofg, last beta (a half near ±pi), the last and current touching flags, 0–4 record entries (kind and +0x24 including exactly 1), and with and without a sound. It checks every word from +0x470 to +0x50c, +0x420 and the touchdown sound count, and that `readings` equals what S220's `readings_of` reads off the original. Branch counts: 694 betas kept, 2945 sign flips, 1311 touchdowns. |
  | `the_crash_test_is_the_originals` | 10,000 | Terrain extent, position (at, near and past the edges, the centre), velocity, rates, the matrix, reason 0/1/13/2, down, clock, WreckTime (sometimes −1), MinWreckSpeed, the throw scale, touching, surface and struck flags against the live masks, the contact bit, no collision object, no world, the ground height (1 in 50 NaN), the speed (1 in 50 NaN) and the rand seed. The wreck, blade-throw and put-back slots are hooked and logged; `ground_query` is hooked to answer the case's height and log each point. It checks the calls asked for in order, every point handed to the ground (4 lanes), reason, down, clock, throw scale, velocity and rates (4 lanes), and the generator's state. Branch counts: 3221 thrown, 1397 crashed (667 by the rotor in the ground), 1231 put back, 1145 blade throws. |
  | `the_mx_rotor_spins_as_the_originals` | 10,000 | Hidden or not, the wrecked-blade node's bit, angle (up to 1e7) and dt. The node functions are hooked to do nothing. It checks +0xe58. |

- `./kit check -- -p game-sim --test kiowa`: 9 passed. Also run with `cargo test --release -- --nocapture`
  to see the counts, so none of them skipped.
- Mutations that make the tests fail:
  - no beta hysteresis
  - touchdown `t < 1` changed to `<= 1`
  - touchdown without the last-flag test
  - the third spin 0.5 changed to 0.7
  - no struck test
  - `!(speed <= min)` changed to `speed > min` (red after the NaN speeds were added)
  - `!(ground <= y)` changed to `ground > y` (red after the NaN grounds were added)
  - no touching test for the thrown helicopter's landing
  - spinning a hidden rotor
- Mutations that survive:
  - Dropping the ±pi clamp on beta. The table never gives more, so it is bit-identical.
  - Dropping the `+ vf0.x` in the sphere transform. It differs only for a −0 lane.
  - The two hysteresis boundaries, `|beta| <= pi/2` and `< 10°`, against `<` and `<=`. Exact
    equality was not reached: the abeam cases do not land on pi/2 to the bit.
- `./kit audit`: only the existing `plan/DONE.md:519/:533` leaks.

## Not verified / not 1:1

- The wreck (vt+0x1b8), unwreck (vt+0x1bc), blade throw (vt+0x1b0, three rand draws a blade) and
  put-back (vt+0x198) are only reported by `Crashed`. The body changes they make are a follow-up.
- The rotor's collision update and contact search are the caller's: `Strike` takes their answer. Its
  test hooks them and writes the contact bit and struck object directly.
- `ground_query`'s own answer is the caller's (`rotor_in_ground` takes a closure). Its test hooks it.
- The touchdown sound's position (`touchdown_sound`) and the sound itself are not ported. The port
  says only whether it plays.
- The MX measure's three presentation copies are not ported, and the node turns by the rotor angle
  are presentation.
- The test's matrices are random rows, not rotations, and the sphere centres are the live ones.
- No ride or capture was replayed: nothing steps the helicopter yet (S220b).
- `Flight::pilot_flag` keeps its S148 name, though it is the collision record's touching flag.

## Next

S220b, and S220a1 (the wreck and put-back).
