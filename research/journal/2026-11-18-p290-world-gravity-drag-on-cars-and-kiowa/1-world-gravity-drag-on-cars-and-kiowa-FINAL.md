# 1 — world gravity drag on cars and kiowa (FINAL)

Task P290. 2026-11-18.

## What I did

Decision: the World category's Gravity and Air drag are the world's, so they now reach every stock
machine, not only the bike and the biplane. All in `crates/game-sim/src/modifiers.rs`, no step touched:

- **Gravity** on the Kiowa (`Mods::kiowa`) and the three cars (`Mods::car`): the `.phy`'s
  `[Mass Properties]` part gets `GravityScale x k`, and its force is worked out again from the new scale
  as `phy::Part::read` does (`part_gravity`). So the part is exactly what a file with `k` times the
  scale would load. Both rides take gravity off that part when taken; the Kiowa's turn feed-forward and
  its unwreck read the scale, so they follow too.
- **Air drag** on the Kiowa: `CD0` and `LowSpeedDrag` x k (the body's drag along its travel, and the slow
  drag on its level velocity). On the cars: every wing's `BasicDrag` and `InducedDrag` x k, so the wings'
  drag force is k times; lift/downforce left alone.
- Each skipped at 100%, as every other knob. Blurbs and field notes say "every machine".
- New test `crates/openmx/tests/machine_modifiers.rs` `the_worlds_gravity_and_air_drag_reach_the_cars_and_the_kiowa`.
- Followed by P371 (the quad and MX vs. ATV's cars; mid-ride take-up).

Nothing to take from `../opentennis` (no vehicles).

## What the original does

Nothing: modifiers are an addition. The original's numbers are the ones at 100%.

## How it was proved

- `machine_modifiers.rs`: at 100% (and with every other machine's knob moved) each car's and the Kiowa's
  file and 180-tick step stay bit-exact (the existing P272 test, which already sets World knobs to 100).
- New test, on the disc's files and the shell's own rides on a flat floor:
  - cars' rest height at 50/100/200% gravity: Buggy 0.59/0.38/-0.04, Monster 2.53/2.08/1.16,
    Baja 1.11/0.87/0.40 ft;
  - cars' top speed at 400% air drag: Buggy 146 -> 113, Monster 201 -> 125, Baja 216 -> 138 ft/s;
  - Kiowa height after 4 s of Cross at 50/100/200% gravity: 205/178/121 ft;
  - Kiowa top speed at 300% air drag: 130 -> 123 ft/s.

## Not verified / not 1:1

- Not 1:1 by design: a modifier off 100% is our addition; there is no original to match.
- The Kiowa's wreck gravity (`WreckGravity`) is not scaled, as the biplane's is not: a wrecked
  helicopter falls at the file's wreck gravity whatever the knob. A choice, not checked by eye.
- A car or Kiowa already out when the pause changes Gravity keeps its old gravity until its next take:
  the shell's `Garage::modify` copies the Kiowa's aero (so drag is at once) but not the ride's gravity,
  and a car's body copied its gravity when taken. Left for P371 (`vehicle.rs` is not in this task's files).
- MX vs. ATV's quad and five cars still get neither knob (their gravity is the handling mass part's,
  not a `phy::Part`): P371.
- The Kiowa's air-drag effect on top speed is small (5% at 300%), since its controller's speed command
  holds the top; not tuned further.

## Next

Nothing for this task. P371 for the rest.
