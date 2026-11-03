# C21 — the thrown blades against the Kiowa's hull

2026-11-03. Task: `./kit plan C21`. This is C19's follow-up.

**What holds.** `a_captured_rotor_strike_replays_bit_exact` (`crates/game-sim/tests/kiowa_capture.rs`)
now compares every row of the wrecked fall and after the put-back, 1299..=1502, bit for bit. Before,
it stopped at 1298.

- The sideways push on 1299..=1302 is the four thrown blades meeting the hull. Each blade's ray
  probe strikes the helicopter's collidable, `kiowaf.col`. The blade gets a kind-0 entry and the hull
  gets the kind-1 mirror. The heli's own record holds nothing else on those frames.
- The port now offers the hull to the blades' search (`Blades::search_on`) and files the mirrors on
  the heli's record (`Collision::file_mirrors`). The blades step inside the heli's world step, after
  it on every list (`Surroundings::among`).
- Rows 1299..=1302 (the push) match after their own frame. So do the fall, the touchdown on 1310
  and the settle on the skids.
- From 1373 on, many rows were read by PINE inside the next frame's world step. The test accepts
  such a row only if it equals one of the port's substeps of that next frame, bit for bit (see
  `1-blades-on-the-hull-FINAL.md`).
- On the put-back (1449) the port runs the frame once more on the placed body, with the pad that
  frame's input step read, as the original does. From 1474 some rows were read inside their frame's
  post, part-way through the measure. The test checks those against a cut of `Measure::flight`'s
  writes.
- The shell (`KiowaRide::step`, `crates/openmx/src/vehicle.rs`) runs the blades the same way and
  loads `kiowaf.col` (`crates/openmx/src/main.rs`).

**Negative control.** Without the hull mesh, the test fails on 1299: the speed is 4.1156425 in the
game and 4.1125774 in the port.

See `1-blades-on-the-hull-FINAL.md`.

## Not verified / not 1:1

See the list at the end of `1-blades-on-the-hull-FINAL.md`. In short:

- the heli's own rays on the blades and blade-against-blade contacts are not ported;
- the 34 part-way rows' accumulators are not compared;
- the reading of the 25 in-post rows is inferred.
