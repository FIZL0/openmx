# 1 — cars and kiowa wrap at seams (FINAL)

Task MD120. 2026-11-12.

## What I did

- `car::Machine::wrap_round` (new): on a world that wraps, a car whose body is past a seam is moved
  back one width — its body (position, last position), its matrix's origin, each wheel's places
  (`upper`, `lower`, `ground_point`, `s.contact`) and its collision object's record (`seam::car`,
  `seam::collision`, `seam::past`, all new in `seam.rs`). It runs first in the car's post listener,
  before the refill, as `Flight::wrap_round` does for the plane, so both `Machine::step` and
  `Machine::step_met` wrap.
- The car's wreck timer edge test asks `off_map::outside_axes` with `world.wrap()` on a world that
  wraps; on every other world it is the original's `Area::outside`, unchanged.
- Kiowa: `kiowa::Surroundings` gets two default methods, `wrap_round(&mut Body)` (`[0, 0]`) and
  `wrap()` (`[false, false]`). `Grounded` implements both off its world (body + collision record).
  The shell's `KiowaAround` forwards them. `Lists::post` calls `wrap_round` first, then passes the
  crash step (MX Unleashed's and SLUS-21104's) the area only when `outside_axes` says it is out on an
  axis that does not wrap, exactly as `Flight::post` does for the plane.
- Shell (`main.rs`): `seat_seam` has a branch for a driven car, measured from `before_cars`. The car's
  spot before the tick and the camera get carried over the seam. The parked bike and plane are left
  where they are (`seam_carry` with no plane slot).
- Test `crates/openmx/tests/wrap_machines.rs`: a flat 1,134 ft floor wrapped on x, each car (buggy,
  monster, baja) on the gas and the Kiowa climbing with forward asked, from x = 1050 (inside the
  150 ft edge) heading east. On the wrapped floor none is thrown, each crosses and comes in at x in
  [0, 1134) still moving, and the Kiowa's matrix and record come with it. On the same floor not
  wrapped, each is thrown (the control).
- Nothing taken from opentennis: it has no wrapped worlds.

## What the original does

Nothing here: wrapping is the port's own (MD29/MD44). With no wrap the car and Kiowa code paths are
the original's, unchanged: `wrap_round` returns at once and the edge test is the old call.

## How it was proved

`cargo test -p openmx --test wrap_machines`: 2 tests, both pass, off the disc's machine.res. Before
this change every machine starting at x = 1050 is inside the old edge margin, so it would have been
thrown on its first tick. The unwrapped control shows the edge still throws.

## Not verified / not 1:1

- Not seen on screen on world_mxu. No car or Kiowa was driven to a real seam in the app. The proof is
  the synthetic flat floor, and the shell's camera carry for a car (`seat_seam`'s new branch) is not
  run by any test. Reason: getting a car across world_mxu's thousands of feet by tape is a long
  drive, and the floor test covers the sim behaviour.
- The car's wrapped crossing is not compared bit for bit with the same drive on a twice-as-wide
  floor, as `wrap_rest.rs` does for the plane. Each crossing is checked only for not being thrown,
  coming in inside the world, and still moving.
- A wrecked Kiowa's thrown blades are not wrapped. They are separate bodies, and their edge is still
  the world rectangle (`prop_world::Edge`, set in `KiowaRide::built`), so a blade thrown over a seam
  stops at the edge instead of wrapping. Not asked for in this task.
- Online: no netplay run with a car over a seam. The wire carries no new fields; the wrap is a pure
  function of the state.
- The edge throw's target, on an axis that does not wrap, is still the area's centre on both axes,
  as the plane's is. That is the original's arithmetic, never reached on world_mxu's axes, which
  both wrap.
- `collider.cofg_world` is not moved by `seam::collision`. It is the centre of mass turned into the
  world (an offset that every contact works out again before reading it), not a place.
  `Flight::wrap_round`, `seam::crash` and `Car::pushed` do move it. That is harmless for the same
  reason, but it treats an offset as a place; filed as a low follow-up.

## Next

Nothing for this task. K52 tidies the cofg_world moves.
