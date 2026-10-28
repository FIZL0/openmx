# 1 — physics props (FINAL)

Task S169. 2026-10-28.

## What I did

- **Found what a "HasPhysics prop" is on the disc.** I dumped every `.res`'s `.toy` (a throwaway
  ignored test, since deleted) and searched them:
  - `HasPhysics` appears only in `[CarN]` sections: free06 Car22–24, free07 Car1–4, free09 Car7,
    free10 Car1–4, free11 Car1–3 and Car8, and the `_ss` twins of each.
  - These are S127's physics cars. They are path cars until struck, their mesh's mover is the car,
    and they already have a `traffic::Moving` (S127 proved its velocity and spin against vt+0xb8/0xbc).
    Their struck mode is S186.
  - No `[AnimatedToyN]` or other toy section has a physics key. The other rigid-body props are the
    48 `.toy`s' `[PhysicsCollections]`: hay bales (`bale_single*`, `up_bale_triple*`, …),
    `banner*`, `SH_CrateC`, `SH_Barrel`, `trash_can`, `sign_*`, `recycling_can`, `tool_box`,
    `BeachBall`, `Water_heater`, free06's `EyeBall`. S80/S130/S131/S198 load and simulate them.
- **Ported a prop as the other side of a contact** (`crates/game-sim/src/prop_world.rs`):
  - `Collection::other(k, f)` hands a closure the `contact::Other` of body `k`: its collision
    object's collider and its body, as `Side::Body`.
  - `Collection::under(k, point)` is what a wheel standing on that prop reads: `contact::other_at`,
    the body's velocity and its velocity at the point. It writes the object's world centre of mass
    on the way, as the original does.
  - `PropSet::under(c, k, point)` is the same for the ride's set.
  - Nothing calls them yet (see Not verified).
- **New test file** `crates/game-sim/tests/prop_under.rs`.
- Added a comment block to `research/oracle.txt`, after `toy_cars_read`. It has no new name lines.
- Filed S207: put the props' `f.col` in the wheel search and read `PropSet::under`, after S197 and S168.
- `world.rs` and `toys.rs` (named in the brief) needed nothing. The animated toys' parts already
  move (S145), and a prop's motion lives in its collection, not in the world's `Props`.

Nothing was taken from opentennis; it has no rigid-body props.

## What the original does

- The physics object's init (`./kit fn 0x001597b0`, read in S197's journal) loads `<model>f.col` into
  the collision object's `+0x1f8` and writes the mesh's `+0x80` = the collision object. Read in
  first_recording's RAM, on every one of free09's 62 props:
  - the `f.col`'s `+0x7c` (mover) = 0, and its `+0x80` (owner) = the physics object's `+0x120`;
  - the collision object's `+0xf4` = the physics object's body (`+0x110`), and its `+0xf0` (a
    vehicle's chassis) = 0.
- So a contact on a prop meets a collision object on a dynamics body, never a vehicle. For a wheel
  this is the contact-velocity step's owner branch (`./kit fn 0x0025b710`, `+0x1f0` set, with no
  vehicle at `+0x1f4`):
  - `+0xa0` = the object's cofg turned by the body's quaternion;
  - `+0x1b0` = the body's velocity;
  - `+0x1c0` = the velocity plus the turned (rates × arm), where the arm is the point's offset from
    the body's position plus `+0xa0`, unturned into the body's frame.
  - S145 ported this branch (`contact::other_at`) against synthetic objects.
- The body class and its update are S130's (`prop_world`, the collection world's step), and the
  collision object is S198's. Neither was re-read here.

## How it was proved

Both tests are in `crates/game-sim/tests/prop_under.rs`, on first_recording's state and the disc's free09.

- `a_props_collidable_names_its_body_not_a_mover`: a save-state check over all 62 free09 props. Each
  `f.col`'s mover is 0 and its owner is the collision object. The object names the prop's body and no
  chassis. The port's loaded prop has a collision object whose centre of mass equals `+0x90` bit for bit.
- `a_wheel_on_a_moving_prop_reads_it_as_the_originals_does`: a function oracle over 10,000 generated
  wheels, every one of the 62 props stood on at least once.
  - Each case writes a generated body position, velocity, unit quaternion and spin into the prop's
    real body in RAM and into the port's loaded `Bone`.
  - A wheel and a chassis are built in free scratch, which is checked to be all zero in the state.
    The wheel names the prop's real collision object as owner, and the chassis hangs on the prop's
    model node.
  - The original's 0x0025b710 runs in the EE interpreter. It is compared with `PropSet::under`
    followed by the ride's `contact_velocity` / `wheel_contact_velocity` / `chassis::unrotate`.
  - `+0xa0`, `+0x1b0`, `+0x1c0`, `+0x170`, `+0x180`, `+0x190` and `+0x1a0` match bit for bit.
  - A mutation run (the port's spin lanes swapped) went red on case 0 at `+0x1c0`, so the comparison bites.
- `./kit check --all` in `./kit done`.

## Not verified / not 1:1

- **Nothing in a ride rides or is pushed by a moving prop yet.** The task's symptom stays as it is in
  play:
  - a prop's `f.col` is not a collidable any search meets (S197, blocked on S198/S195);
  - the wheel search does not list it or set the owner (S168 for other riders' bikes, S207 for props).
  - What is ported is what such a contact reads off the prop, proved as a function. The wiring is in
    those tasks.
- **A body contact (a ray of the bike or rider body) on a moving prop** would read the prop through the
  same `Other` in `contact::resolve` (`Collection::other` gives it). Whether the original's body-ray
  search hands it the object, or the response's mover/actor choice, was not checked. That is S197's.
- **Only free09's props were checked in RAM.** No state of another track's collections was at hand. The
  `f.col` init is shared code, so the other tracks should be the same, but this was not seen.
- **The HasPhysics cars were not re-proved.** Their `Moving` (unstruck) is S127's proof, and their struck
  rigid-body mode, where a struck car would stop giving a `Moving` and become a body, is S186.
- **The generated bodies are not states the game reached.** The poses, velocities and spins are drawn
  around each prop's rest place, not taken from a live knock. The arithmetic is the same for any
  inputs, but no captured moving prop was compared (S195 has no capture yet).

## Next

S197 (props struck), then S207 (wheels on props).
