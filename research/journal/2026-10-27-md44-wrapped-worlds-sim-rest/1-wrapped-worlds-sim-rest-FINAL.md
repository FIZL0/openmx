# 1 — wrapped worlds sim rest (FINAL)

Task MD44. 2026-10-27.

## What I did

MD29 shifted the ridden chassis over a seam. This task does the same for the other bodies that move across one.

- **`crates/game-sim/src/seam.rs` (new).** Small functions that move each kind of body's *places* along x/z
  by a width: `body` (position and last_position), `record` (node origin, both sweep ends, contact points),
  `ragdoll` (bones, plus targets of pins that hold to the world), `actor`, `riding` (the actor plus drawn/was
  matrices), `down` (a wreck's ragdoll, bone contacts, and the bike search's stood point and centre), and
  `crash` (wreck bike body, record, cofg, origin, rider point). They leave velocities, rates, orientations
  and anything in a body's own frame alone. `lap`/`by` are the seam test, moved out of `Rider::wrap_round`.
- **Rider.** `wrap_round` no longer waits for the rider to be plainly riding. During a wreck the wreck bike's
  position decides which side of the seam the rider is on. `Rider::shift` also moves `riding`, `crash`,
  `ragdoll` and `ragdoll_shown`.
- **Plane.** `Flight::wrap_round` shifts the plane's body, record and collider cofg, then remeasures. It runs
  at the start of the post listener, before `refresh`, so the nearby list is gathered again at the new
  place. `frame_seamed` passes the seam back, and `Sim::step`'s plane branch now returns it in
  `Frame.seam` like a ride does. Bikes standing in the plane's world are placed at the copy nearest the
  plane. The `Flight` struct is unchanged, so the snapshot wire is unchanged too.
- **Bike-to-bike contacts.** `rider_contact::gather_near` places each other seat at the copy nearest "me"
  before colliding, then moves mirrored contacts back to the struck bike's own copy. `bump_of` resolves
  against other vehicles at their nearest copy as well. `Sim::bike_contacts_in` / `Sim::bump_in` take the
  world, and `step_all` uses them. `gather`/`bump` keep their old behaviour by using the identity `here`.

Nothing from opentennis applied: it has no wrapping world.

## What the original does

Nothing: the original's worlds do not wrap. This is the port's own (MD29's switch, off by default).

## How it was proved

`crates/game-sim/tests/wrap_rest.rs`, 4 tests. MD29's `tests/wrap.rs` (9 tests) still passes.

- **Plane over the seam.** The plane is launched east at 150 ft/s in a two-copy wrapped world. Until it
  crosses, its hashes match the same flight in a wide unwrapped world. On the crossing tick, position,
  last_position, record origin and collider cofg are bit-exact, less the width. Velocity, rates, quaternion,
  airspeed, measured velocity, attitude and air are equal. The matrix origin is within 1e-3 because
  remeasure recomputes it. For 60 more ticks it stays within 0.05 ft / 0.01 ft/s of the wide flight.
- **Wreck over the seam.** A rider rolled off 16 ft up turns into an air wreck crossing the seam. On the
  crossing tick, every place in the wreck is bit-exact against a clone stepped without the wrap, less the
  width. Velocities and quaternions are equal, and the put-back x is inside the world.
- **Strikes across a seam.** Seats at x 0.75 and 1133.25 give bike 0 the same kind-0 contacts, bit for bit,
  as seats at 0.75 and −0.75. Bike 1's contacts are within 1e-3 and sit a width over.
- **Two riders a seam apart touch.** A race125p pair at 0.75/1133.25 is ridden 30 ticks beside a pair at
  567.75/566.25. The contact counts match on every tick, and plain `bike_contacts` finds none at tick 0.
  The worst gap between the two pairs is 0.005 ft. The pair gets pushed 2.07 ft away from where each bike
  would ride alone, and the spread grows from 1.5 to 4.46 ft.

## Not verified / not 1:1

- **Riding ragdoll shift (OPENMX_RIDING_RAGDOLL=1) has no test.** `seam::riding` is only read through. A
  test needs the disc's rider kit (`ragdoll_ride::Kit` from the game_render rider body) and the shell's
  holds, which is more setup than this task justified.
- **`Rider::holds` and `skeleton` are not shifted.** The shell supplies them each frame, so they are
  inputs, not state.
- **Car and traffic contacts are not taken across a seam.** Only bike-to-bike contacts are: the brief
  named those.
- **`among_in` / `bump_among_in` have no caller.** I added them for online seats, but netplay does not call
  `among`.
- **The plane's nearby lists and broad-phase centres are not shifted.** The post listener's `refresh`
  gathers them again on the same substep, after the shift.
- **The edge throw on a wrapped axis is unchanged.** A cannonball still throws "toward the middle", which
  depends on the width. That is MD29's behaviour, and I did not revisit it.
- **The shell's camera does not use the plane's `Frame.seam`.** The draw is MD43's.
- **`ragdoll_shown` is shifted but not checked.** It is only drawn, so this is untested.

## Next

Nothing. MD43 draws across the seam and can read the plane's `Frame.seam`.
