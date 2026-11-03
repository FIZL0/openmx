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
- On the put-back (1449) the port ran the frame once more on the placed body. R26 (below) found
  that was the capture reading a frame late, and removed it. From 1474 some rows were read inside a
  frame's post, part-way through the measure. The test checks those against a cut of
  `Measure::flight`'s writes.
- The shell (`KiowaRide::step`, `crates/openmx/src/vehicle.rs`) runs the blades the same way and
  loads `kiowaf.col` (`crates/openmx/src/main.rs`).

**Negative control.** Without the hull mesh, the test fails on 1299: the speed is 4.1156425 in the
game and 4.1125774 in the port.

See `1-blades-on-the-hull-FINAL.md`.

## C24 — the heli's rays on its blades

See `2-c24-the-hulls-rays-on-the-blades-FINAL.md`. In eight captured wrecks the original's heli never
files a kind-0 entry on a blade, and no blade files one on another. The heli's kept nearby list is
gathered afresh only when its probe leaves the list's bound, so a wreck that stays put never holds its
blades. The port now offers the blades to the heli's rays only while that list holds them
(`Blades::gather_round`, `Collision::search_knocking`). Three new replays (a roll, a tip, a fast
slide) are bit-exact to their captures' ends.

## R26 — the put-back steps once

See `3-r26-the-put-back-steps-once-FINAL.md`. The original steps the placed body once, from the next
frame, as in C24's eight wrecks. C21's capture reads later and later through the fall: from 1373 into
the next frame's substeps, and from the put-back on a whole frame late. So its row 1449 holds frame
1450's state. The C21 replay now compares those rows after the next frame. The shell's second step
is gone.

## B23 — try5 reads a row late

See `4-b23-try5-reads-a-row-late-FINAL.md`. C24's try5 parted on 1304 because its row for 1304 was
read a frame late: it holds frame 1305's body. Compared there, try5 replays bit-exact to 1488. The
contact counts it showed short over 1297..1302 are rows read while the collide pass was refilling the
record. The body is bit-exact on every one of them.

## B24 — fast2 reads ever later

See `5-b24-fast2-reads-ever-later-FINAL.md`. C24's fast2 parted on 1416, in flight, because that row
was read inside frame 1417's first force listener. Its commands are the port's from that listener.
The replay now compares each row where it was read: part-way through the next frame's controller and
rotor, or after the next frame. Read that way, fast2 is bit-exact from 1205 to 1671, through its
wreck (1442) and put-back (1593).

## Not verified / not 1:1

See the list at the end of `1-blades-on-the-hull-FINAL.md`. In short:

- the heli's own rays on the blades are ported (C24) but never seen meeting one; blade-against-blade
  contacts are not ported;
- the 34 part-way rows' accumulators are not compared;
- the reading of the 25 in-post rows is inferred.
