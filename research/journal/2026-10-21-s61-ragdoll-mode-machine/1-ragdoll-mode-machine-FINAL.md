# 1 — ragdoll mode machine (FINAL)

Task S61. 2026-10-21.

## What I did

- **`crates/game-sim/src/ragdoll_body.rs`** (new): `Actor::tick` is the rider body actor's update
  around the ragdoll's step. In order, it runs:
  - the request's effects and the two look clocks;
  - the mode machine (placed / gate / wreck / held / riding, each with its entry);
  - the wreck arm: throw clock, hands letting go, touching and roll, off-bike, ground query, the
    settle test with the settle/flung damping switch, and the torques' fade countdown;
  - the collision flags cleared, the hold targets and velocity, the bike's box and frame;
  - the physics update: the step when placed, active and not reposing, then every bone quaternion
    normalised, then the repose pose;
  - the tail: touching, the summed impact and the main bone's place.
  
  The world, the scene and the skeleton's pose come in through `Scenery` and the `Outside` trait,
  so a test can hand it the original's own.
- **`ragdoll::product`**: a public quaternion product, which the main bone's orientation outside the
  wreck needs.
- **`ragdoll_ride::Down`** now runs the ported wreck arm (`ragdoll_body::wreck_arm`) each step. That
  brings in the throw clock, touching, roll, ground query, settle test and fade, with hold velocity
  = the bike's ×0.8, the collision flags cleared before the step and the quaternions normalised
  after it. The ride's kit carries the rider file's `[Wrecks]`. `Down` gains `fade` and
  `touching`, which are wired into snapshots and the hash walk. `rider.rs` no longer syncs the
  damping by hand, because the settle test throws its own switch.
- **`Down::settles` is unchanged** (S53's hands off and a bone touching). The brief said to replace
  the inferred gate. I tried the body's own settle flag in its place, and S53's reason-5 test moved
  from 16 to 39 ticks (the original's is 18). S26 had already seen the ride machine's gate
  (`+0xd3c` on the rider) come on with the body still at 42 ft/s, so it is not the settle test.
  Its writer is still unknown. Follow-up **S65**.
- `tests/hash.rs`: the `Sim` size pin goes 9472 → 9480.

## What the original does

See research/oracle.txt, "S61". In short:
- The mode machine reads the main bone's rest *before* the skeleton updates; the step reads the
  rests *after* it.
- The impact sum is `vadd.xyz` (its w stays 1).
- Every stiffening restarts the torques' fade.
- The settle switch draws no random number. The draw S56 saw on the settle tick is the animation
  selection.

## How it was proved

This is an Ee oracle over the s56 save-state captures (`context/capture/s56/{front,side,back,tip}`),
in `crates/game-sim/tests/ragdoll_wrecks.rs`. From each tick's RAM, the port's `Actor::tick` is
compared with the original's own tick on the Ee. The world, scene and skeleton calls are hooked, so
the port is handed exactly what the original read. The compare covers every bone lane, pin, joint,
contact, flag, launch/pull, mode, fade, hold target/velocity, bike box and frame, the pose damping
and the impact.

- `the_ports_tick_is_the_originals`: the 35 ticks the S56 replay holds bit-exact (1 entry, 28
  touching, 3 off-bike). All equal.
- `the_ports_tick_is_the_originals_to_rest`: the Ee then runs free to the end of each capture, with
  the bike's side laid in from the capture. That is 874 ticks: 841 touching, 33 flung, 1 landing,
  4 settles (one per capture) and 677 settled. Every compared number is bit-equal. 12 ticks are not
  compared, the ones with a moving collider's hit (see below). The test asserts this coverage.

`./kit check` passes.

## Not verified / not 1:1

- **Ticks with a moving collider's hit (12 of them).** On these the sphere record's +0x90 flag is
  set. They are skipped, because the original's push-out off something moving reads actors the
  capture does not carry.
- **The look block and the feet** (look clocks, look target easing, foot holds and the foot fade).
  These are ported but never ran differently from rest in the captures, because no look-around or
  foot hold was live in a wreck.
- **The model effects** (request +0x238/+0x23c): ported, but all zero in the captures.
- **The non-wreck arms**: the placed and gate entries, the start-line cut branch, and riding
  (mode 1) with its entry. Held (mode 3) ticks after front's wreck are compared and pass. The others
  never occur in the s56 captures. They are a reading of the disassembly with no oracle behind it.
- **The ride**: `ragdoll_ride::Down` still poses from the Rig's fixed pose (not the skeleton) and
  hands the wreck arm no bike, so `off_bike` never sets in a ride. The fade length is the constant 1.
  The ride itself is not compared with the original. Follow-up S65.
- **The ride machine's settle gate** stays S53's inference (see above). The exact-tick sibling test
  stays ignored.
- **The broad phase and the scene** are not ported here: the test hands the port the original's own
  ground, sphere and segment answers.
- **Off-bike** is the flag only: what the original detaches when it sets was not followed.
- **Linear acceleration w** (and angular acceleration w on the pinned body) is a stale register, so
  it is not compared, as in tests/ragdoll.rs.

## Next

Landed. S65 wires the actor into the ride.
