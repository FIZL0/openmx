# 1 — truck mounts and inertia (FINAL)

Task S508. 2026-11-23.

## What I did

- Put the Sim's truck beside the original's per substep: a temporary print in the Sim just before the
  drivetrain step, and a hook on the original's engine step (0x399e40) in the node update 0x26f800,
  replayed on states/r48/r48-1487.ram. Every field was diffed. Both prints were removed.
- Found two causes of the last difference (S489 left it open):
  1. The tire mounts (tire +0x80) differed before any force.
  2. The body's inverse inertia (+0x120) and the collision object's copy differed in the last digits.
- `seat_truck` in crates/game-sim/tests/c16_fly.rs now takes the mounts and both inertias off the
  image, as tests/car_spawn.rs already does for mounts and body inertia.
- `r48_the_plane_keeps_pushing_the_truck` now asserts position and velocity (all axes) of the plane
  and the truck against the capture rows 1570 to 1640. `R48_PUSH_THROUGH` is 1640, was 1604.
- research/r48/index.md: "The last difference (S508)" added after S489's section.
- No shipped code changed.

## What the original does

- The tire mounts (tire +0x80, tires at car +0x548 + 4k) come off the placed model. The placed
  model's round trip rounds them.
- The body's inverse inertia (+0x120) and the collision object's copy (car +0x170 object, +0x80) carry
  the same rounding.

## How it was proved

- Per substep, the Sim's truck against the original's RAM at its engine step, for rows 1569 and 1570.
  The Sim matched the capture through row 1568. The first difference is inside row 1569's first
  substep, not at 1572.
  - Tire lever x: -6.19055 Sim, -6.1901364 original. The other tire: +6.1905794 and +6.191113.
    y differed by 1.7e-6, z by up to 2e-4.
  - Inverse inertia: (9.880182e-5, 9.034243e-5, 1.3870311e-4) Sim, (9.880171e-5, 9.034181e-5,
    1.3870181e-4) original.
- With only the mounts and the inertia seated, the truck and plane stay within 0.0007 ft/s through
  row 1610. The wing ray at 1587 is met.
- `cargo test --release -p game-sim --test c16_fly`: all 19 pass, including R53's with their results
  unchanged.

## Not verified / not 1:1

- Machine::new still builds the mounts and inertia from the unplaced model. A truck in the shipped
  game is about 0.0005 ft off in its mounts. Only the tests seat the original's values. Reason: this
  task changed the test only. Filed as S512.
- Row 1569 is torn in the capture, so only the existing meeting test holds it.

## Next

- S512: port the placed-model round trip for mounts and inertia in Machine::new.
