# The three controllers, diffed and proved

## What the diff found

- **Stand-upright.** The same as MX Unleashed instruction for instruction in its arithmetic. Its
  sources differ: the guard is the rider's stunt differing from the balance stunt (the port's
  `Step::balancing`, which is also what calls the balance), the gravity is read through the bike's
  physics-world pointer, and the forward axis comes out of the body's quaternion (`quat_axes`, `w` 1)
  instead of the chassis transform. The force goes through the body's add-force, which is the same
  `vadd.xyz` into the force accumulator.
- **Balance.** Clause for clause the same. The listing does look different: every integration is
  `adda 0, x` then `madd`, the flattened normal's squares are summed `y·y` first, and the arc cosine
  weighs its upper entry first. But ps2-float's `madd` is `add(acc, mul(a, b))` and a single add is
  commutative, so each of those adds the same two rounded terms as `move.rs`'s order. I first wrote
  them out separately in `step.rs`. Mutation runs that swapped them back stayed green, and that is
  how I found they were one computation, so the port now shares `move.rs`. The real difference is the
  front normal: it comes into the bike by the body quaternion's conjugate rotation, inline VU0 code
  equal to `quad::Body::to_local`, not by the node's transform.
- **Roll/pitch.** Clause for clause the same: thresholds, targets, fade, latch, the zeroed wheelie
  yaw and the rear normal for the stoppie's yaw. The yaw moments go through the body's world-moment
  call (into the body by its quaternion), and nothing tests for a transform first, so `step::roll_pitch`
  sets `has_transform` for the call and puts it back.

## How it was checked

`tests/mxvatv_step.rs` has three new oracles: `mxvatv_stand_upright_is_the_originals`,
`mxvatv_balance_is_the_originals` and `mxvatv_roll_pitch_is_the_originals`. Each pokes 10,000 random
states into the bike, body, engine and both wheels and calls the original (only the smooth noise is
hooked, the same table on both sides). It leaves the quaternion-axes, add-force, add-moment and
world-moment calls in, and compares every word of those regions with the port's. The test also checks
that the image's arc cosine table and scale equal `AcosTable::build()`. Path tallies: stand-upright
left alone / held / held and braked ≥ 500 each; balance no front / ground to either side ≥ 500 each;
roll/pitch airborne, wheelie, neither, stoppie starts, stoppie held, stoppie with no top speed ≥ 100
each. A full forward lean got its own weight in the generator so that stoppie starts reach 100.

These mutations each red an oracle: keeping the has-transform test (roll/pitch); rotating by the
quaternion itself instead of its conjugate (balance); the up axis for forward (stand-upright);
`(dt·500)·err` in `move.rs`' pitch integral (roll/pitch); a nudged cosine in `move.rs`' roll angle
(balance).

`./kit check -- -p game-sim --test mxvatv_step --test mxvatv_ride --test mxvatv_air --test move
--test hash --test levers --test snapshot`: 57 passed. `cargo build -p openmx` also built.

## Not verified / not 1:1

- No MX vs. ATV ride capture exists, so no whole ride has been replayed through these controllers.
- The stunt comparison (rider stunt vs. balance stunt) is mapped to `Step::balancing` by analogy with
  when the balance runs. MD63a mapped the limiter's same comparison to `hands_off`. The stunt words
  themselves are not ported.
- The quaternion the port hands in is `chassis.quat` as the ride's integrate leaves it each substep.
  That integrate is MX Unleashed's, not compared against MX vs. ATV's.
- The smooth noise is hooked on both sides, not compared. The body's force/moment listeners
  (body+0xe8) were empty in the oracle.
- Body+0x1a0 is the centre of gravity by its role in the balance, not by a trace of its readers.
